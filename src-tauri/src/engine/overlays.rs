//! Per-champion overlays: a champion's own settings go on top of the account's base
//! when that champion is picked, and come off again after the game.

use serde_json::Value;

use super::actions::{read_settings, user_changes, Announcement, Result};
use super::Engine;
use crate::profiles::Overlay;
use crate::settings::merge;

/// Swiftplay. It has no real champ select: the champion is chosen in the lobby and the
/// game launches about a second after the champ-select session appears.
const SWIFTPLAY_QUEUE: i64 = 480;

/// What a champion becoming the local player's pick calls for.
#[derive(Debug, PartialEq)]
pub(super) enum Step {
    /// Put this champion's overlay on.
    Apply,
    /// Another champion's overlay is on and this one has none: back to the base.
    Restore,
    Nothing,
}

pub(super) fn on_pick(active: Option<u32>, champion: u32, has_overlay: bool) -> Step {
    match (has_overlay, active) {
        (true, Some(active)) if active == champion => Step::Nothing,
        (true, _) => Step::Apply,
        (false, Some(_)) => Step::Restore,
        (false, None) => Step::Nothing,
    }
}

/// The local player's champion in a champ-select session, once they have one. Draft
/// and blind pick set it at lock-in; ARAM sets it from the start and on every swap.
pub(super) fn picked_champion(session: &Value) -> Option<u32> {
    let cell = session.get("localPlayerCellId")?.as_i64()?;
    let me = session.get("myTeam")?.as_array()?.iter().find(|player| player.get("cellId").and_then(Value::as_i64) == Some(cell))?;
    let champion = u32::try_from(me.get("championId")?.as_i64()?).ok()?;
    (champion != 0).then_some(champion)
}

/// The first champion slot of a Swiftplay lobby. Which of the two slots the player gets
/// is only known a second before launch; the first is the likelier one, and the
/// champ-select session corrects it if need be.
pub(super) fn swiftplay_champion(lobby: &Value) -> Option<u32> {
    if lobby.pointer("/gameConfig/queueId")?.as_i64()? != SWIFTPLAY_QUEUE {
        return None;
    }
    let champion = u32::try_from(lobby.pointer("/localMember/playerSlots/0/championId")?.as_i64()?).ok()?;
    (champion != 0).then_some(champion)
}

impl Engine {
    /// The local player's pick became `champion`. Runs on every change, so that rerolls,
    /// swaps and trades end with the right settings; the latest one wins.
    pub(super) async fn on_champion(self, champion: u32) {
        *self.inner.last_champion.lock().unwrap() = Some(champion);
        if let Err(err) = self.apply_overlay_for(champion).await {
            tracing::warn!(champion, "could not switch champion settings: {err}");
        }
    }

    async fn apply_overlay_for(&self, champion: u32) -> Result<()> {
        let _guard = self.inner.action_lock.lock().await;
        let Some(account) = self.account() else { return Ok(()) };
        // No baseline, no base to come back to.
        let Some(baseline) = account.baseline.clone() else { return Ok(()) };
        let overlay = self.inner.store.load_overlay(champion)?.filter(|overlay| !overlay.settings.is_empty());

        match on_pick(account.overlay, champion, overlay.is_some()) {
            Step::Nothing => Ok(()),
            Step::Restore => self.restore_base_locked("another champion was picked").await,
            Step::Apply => {
                // Settings the user changed and has not settled yet must not be
                // overwritten; the prompt about them comes first.
                let expected = self.expected(&account)?.unwrap_or_else(|| baseline.clone());
                if !user_changes(&expected, &read_settings(&self.connection()?)?).is_empty() {
                    tracing::info!(champion, "not applying champion settings over unsettled changes");
                    return Ok(());
                }
                let overlay = overlay.expect("Step::Apply implies an overlay");
                let (applied, _) = self.write_settings(&merge(&baseline, &overlay.settings), None).await?;
                self.update_account(|account| account.overlay = Some(champion))?;
                let name = self.inner.champions.name(champion).unwrap_or_else(|| format!("champion {champion}"));
                tracing::info!(champion, %name, changed = applied.changed, "champion settings on");
                let _ = self.inner.notices.send(Announcement::Notice(format!("{name}'s settings are on for this game")));
                Ok(())
            }
        }
    }

    /// Takes a champion's overlay off again, if one is on.
    pub(super) async fn restore_base(&self, why: &str) -> Result<()> {
        let _guard = self.inner.action_lock.lock().await;
        self.restore_base_locked(why).await
    }

    async fn restore_base_locked(&self, why: &str) -> Result<()> {
        let Some(account) = self.account() else { return Ok(()) };
        let (Some(champion), Some(baseline)) = (account.overlay, account.baseline) else { return Ok(()) };
        let (applied, _) = self.write_settings(&baseline, None).await?;
        self.update_account(|account| account.overlay = None)?;
        tracing::info!(champion, changed = applied.changed, why, "champion settings off");
        Ok(())
    }

    /// Queueing started. In Swiftplay this is the moment to act, as the champion is
    /// already chosen and champ select will be too short.
    pub(super) async fn on_matchmaking(self) {
        let Ok(connection) = self.connection() else { return };
        match connection.client.get::<Value>("/lol-lobby/v2/lobby").await {
            Ok(lobby) => {
                if let Some(champion) = swiftplay_champion(&lobby) {
                    tracing::info!(champion, "Swiftplay: applying the first slot's champion settings while in queue");
                    self.on_champion(champion).await;
                }
            }
            Err(err) => tracing::debug!("could not read the lobby: {err}"),
        }
    }

    /// Champ select or the queue ended without a game.
    pub(super) async fn on_no_game(self) {
        if let Err(err) = self.restore_base("there was no game").await {
            tracing::warn!("could not take the champion's settings off again: {err}");
        }
    }

    /// Every champion overlay that overrides something, by champion id.
    pub fn overlays(&self) -> Result<Vec<Overlay>> {
        Ok(self.inner.store.list_overlays()?.into_iter().filter(|overlay| !overlay.settings.is_empty()).collect())
    }

    /// The champion whose overlay is on the logged-in account right now.
    pub fn active_overlay(&self) -> Option<u32> {
        self.account()?.overlay
    }

    /// Deletes a champion's overlay. If it is on right now, it comes off first.
    pub async fn delete_overlay(&self, champion: u32) -> Result<()> {
        let _guard = self.inner.action_lock.lock().await;
        if self.account().and_then(|account| account.overlay) == Some(champion) {
            self.restore_base_locked("its overlay was deleted").await?;
        }
        self.inner.store.delete_overlay(champion)?;
        tracing::info!(champion, "deleted champion overlay");
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_pick_switches_overlays_only_when_needed() {
        // No overlay anywhere: nothing to do.
        assert_eq!(on_pick(None, 157, false), Step::Nothing);
        // Picking a champion with an overlay puts it on.
        assert_eq!(on_pick(None, 157, true), Step::Apply);
        // Already on, as when the session updates without the pick changing.
        assert_eq!(on_pick(Some(157), 157, true), Step::Nothing);
        // Rerolled to another champion with an overlay: switch.
        assert_eq!(on_pick(Some(157), 18, true), Step::Apply);
        // Rerolled to one without: the previous champion's settings must come off.
        assert_eq!(on_pick(Some(157), 18, false), Step::Restore);
    }

    #[test]
    fn reads_the_local_players_pick() {
        let session = json!({
            "localPlayerCellId": 2,
            "myTeam": [
                { "cellId": 1, "championId": 99 },
                { "cellId": 2, "championId": 157, "championPickIntent": 0 },
            ],
        });
        assert_eq!(picked_champion(&session), Some(157));

        // Hovering is not picking: championId stays 0 until lock-in.
        let hovering = json!({ "localPlayerCellId": 2, "myTeam": [{ "cellId": 2, "championId": 0, "championPickIntent": 157 }] });
        assert_eq!(picked_champion(&hovering), None);
        assert_eq!(picked_champion(&json!({})), None);
    }

    #[test]
    fn reads_the_first_swiftplay_slot() {
        let lobby = json!({
            "gameConfig": { "queueId": 480 },
            "localMember": { "playerSlots": [{ "championId": 246 }, { "championId": 145 }] },
        });
        assert_eq!(swiftplay_champion(&lobby), Some(246));

        // Other queues have a real champ select and are left to it.
        let draft = json!({ "gameConfig": { "queueId": 400 }, "localMember": { "playerSlots": [{ "championId": 246 }] } });
        assert_eq!(swiftplay_champion(&draft), None);
        let empty = json!({ "gameConfig": { "queueId": 480 }, "localMember": { "playerSlots": [] } });
        assert_eq!(swiftplay_champion(&empty), None);
    }
}

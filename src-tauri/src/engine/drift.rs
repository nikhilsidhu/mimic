//! Settings the user changed in the game: noticing them, asking where they go, and the
//! settings muted from that question.

use std::time::Duration;

use super::actions::{count, read_settings, user_changes, ActionError, Announcement, Result, KEEP_SNAPSHOTS};
use super::Engine;
use crate::profiles::{Account, Overlay, Snapshot};
use crate::settings::{merge, Change, SettingsMap};

/// How long after a game the settings are left alone before being compared. The game
/// writes them on exit and the client then reloads them.
const AFTER_GAME_SETTLE: Duration = Duration::from_secs(6);

/// Settings the user changed, and where they could be saved: the account's profile, or
/// the champion they were made on.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Drift {
    pub profile: Option<String>,
    pub champion: Option<ChampionRef>,
    pub changes: Vec<Change>,
    /// The changes are Riot's doing: the account's settings were reset, as after a patch.
    pub reset: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ChampionRef {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DriftChoice {
    /// Make the changes part of the account's profile, so other accounts get them.
    SaveToProfile,
    /// Keep the changes for the champion they were made on, as its overlay, and take
    /// them off the account again.
    SaveToChampion,
    /// Put the account back the way it was.
    Revert,
    /// Accept the changes on this account and leave the profile as it is.
    KeepHere,
}

impl Engine {
    /// What the user changed on the logged-in account since mimic last saved, applied
    /// or accepted its settings. `None` when nothing did, or when there is no baseline.
    /// With a champion's overlay on top, the overlay's own values are expected and only
    /// what differs from `baseline ⊕ overlay` counts.
    pub fn drift(&self) -> Result<Option<Drift>> {
        let connection = self.connection()?;
        let Some(account) = self.account() else { return Ok(None) };
        let Some(expected) = self.expected(&account)? else { return Ok(None) };
        let changes = self.asked_about(&expected, &read_settings(&connection)?);
        if changes.is_empty() {
            return Ok(None);
        }
        let profile = match &account.profile_id {
            Some(id) => self.inner.store.load_profile(id)?.map(|profile| profile.name),
            None => None,
        };
        // Changes can be kept for the champion they were made on: the one whose overlay
        // is active, or else the one just played.
        let champion = account.overlay.or(*self.inner.last_champion.lock().unwrap()).map(|id| ChampionRef {
            id,
            name: self.inner.champions.name(id).unwrap_or_else(|| format!("champion {id}")),
        });
        let reset = *self.inner.reset.lock().unwrap();
        Ok(Some(Drift { profile, champion, changes, reset }))
    }

    /// Settings the user does not want to be asked about, as `file/section/key`.
    pub fn muted(&self) -> Vec<String> {
        self.inner.store.load_state().map(|state| state.muted).unwrap_or_default()
    }

    /// Stops asking about a setting. Changes to it are kept on the account silently.
    pub fn mute(&self, id: &str) -> Result<()> {
        let mut state = self.inner.store.load_state()?;
        if !state.muted.iter().any(|muted| muted == id) {
            state.muted.push(id.to_owned());
            state.muted.sort();
            self.inner.store.save_state(&state)?;
        }
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(())
    }

    /// Mutes a setting from the prompt about changed settings. If that leaves nothing to
    /// ask about, the prompt's job is done: a champion's overlay comes off again.
    pub async fn mute_setting(&self, id: &str) -> Result<()> {
        self.mute(id)?;
        if self.connection().is_err() {
            return Ok(());
        }
        {
            let _guard = self.inner.action_lock.lock().await;
            self.absorb_muted()?;
        }
        if self.drift()?.is_none() {
            self.restore_base("the last changed setting was muted").await?;
        }
        Ok(())
    }

    pub fn unmute(&self, id: &str) -> Result<()> {
        let mut state = self.inner.store.load_state()?;
        state.muted.retain(|muted| muted != id);
        self.inner.store.save_state(&state)?;
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(())
    }

    /// What the user changed that they want to be asked about.
    pub(super) fn asked_about(&self, expected: &SettingsMap, current: &SettingsMap) -> Vec<Change> {
        let muted = self.muted();
        user_changes(expected, current).into_iter().filter(|change| !muted.contains(&mute_id(change))).collect()
    }

    /// Takes changes to muted settings into the baseline, so that they are kept and do
    /// not pile up as differences nobody is asked about.
    pub(super) fn absorb_muted(&self) -> Result<()> {
        let connection = self.connection()?;
        let Some(account) = self.account() else { return Ok(()) };
        let (Some(mut baseline), Some(expected)) = (account.baseline.clone(), self.expected(&account)?) else {
            return Ok(());
        };
        let kept = keep_muted(&mut baseline, &expected, &read_settings(&connection)?, &self.muted());
        if kept == 0 {
            return Ok(());
        }
        tracing::info!(settings = kept, "kept changes to muted settings without asking");
        self.update_account(|account| account.baseline = Some(baseline))
    }

    /// What should be on the account if the user changed nothing: its baseline, with
    /// the active champion overlay on top.
    pub(super) fn expected(&self, account: &Account) -> Result<Option<SettingsMap>> {
        let Some(baseline) = &account.baseline else { return Ok(None) };
        let overlay = match account.overlay {
            Some(champion) => self.inner.store.load_overlay(champion)?,
            None => None,
        };
        Ok(Some(match overlay {
            Some(overlay) => merge(baseline, &overlay.settings),
            None => baseline.clone(),
        }))
    }

    /// Runs after a game: gives the game and the client a moment to finish writing the
    /// settings, then asks the user about whatever changed. If nothing did, a champion's
    /// overlay comes off again without a word.
    pub(super) async fn check_drift_after_game(self) {
        tokio::time::sleep(AFTER_GAME_SETTLE).await;
        {
            let _guard = self.inner.action_lock.lock().await;
            if let Err(err) = self.absorb_muted() {
                tracing::warn!("could not keep changes to muted settings: {err}");
            }
        }
        match self.drift() {
            Ok(Some(drift)) => {
                tracing::info!(changes = drift.changes.len(), "settings changed during the game");
                let _ = self.inner.notices.send(Announcement::Drift);
            }
            Ok(None) => {
                if let Err(err) = self.restore_base("the game is over").await {
                    tracing::warn!("could not take the champion's settings off again: {err}");
                }
            }
            Err(err) => tracing::warn!("could not check for changed settings: {err}"),
        }
    }

    /// Settles what [`Engine::drift`] reported. Whatever is chosen, a champion's overlay
    /// comes off afterwards and the account is back on its base.
    pub async fn resolve_drift(&self, choice: DriftChoice) -> Result<String> {
        self.settle_changes(choice, None).await
    }

    /// `for_champion` names the champion for [`DriftChoice::SaveToChampion`]; without it
    /// the one whose overlay is on, or else the one just played, is used.
    pub(super) async fn settle_changes(&self, choice: DriftChoice, for_champion: Option<u32>) -> Result<String> {
        let _guard = self.inner.action_lock.lock().await;
        let connection = self.connection()?;
        let account = self.account().ok_or(ActionError::NotConnected)?;
        let (Some(baseline), Some(expected)) = (account.baseline.clone(), self.expected(&account)?) else {
            return Ok("Nothing to do".to_owned());
        };
        let current = read_settings(&connection)?;
        // Muted settings stay as they are whatever is chosen; the choice is about the rest.
        let mut base = baseline;
        keep_muted(&mut base, &expected, &current, &self.muted());
        let changes = self.asked_about(&expected, &current);
        if changes.is_empty() && choice == DriftChoice::SaveToChampion {
            return Err(ActionError::NothingChanged);
        }
        // A removed key is not a change anyone made.
        let set_changes = |settings: &mut SettingsMap| {
            for change in &changes {
                if let Some(value) = &change.to {
                    settings.set(&change.file, &change.section, &change.key, value);
                }
            }
        };

        // What the base becomes, and what to tell the user.
        let said = match choice {
            DriftChoice::SaveToProfile => {
                let id = account.profile_id.clone().ok_or(ActionError::NoProfile)?;
                let mut profile =
                    self.inner.store.load_profile(&id)?.ok_or_else(|| ActionError::NoSuchProfile(id.clone()))?;
                // Only what changed goes in, so that values the client refused on this
                // account do not leak into the profile.
                set_changes(&mut profile.settings);
                profile.updated = time::OffsetDateTime::now_utc();
                self.inner.store.save_profile(&profile)?;
                set_changes(&mut base);
                format!("Saved {} to '{}'", count(changes.len(), "change"), profile.name)
            }
            DriftChoice::SaveToChampion => {
                let champion = for_champion
                    .or(account.overlay)
                    .or(*self.inner.last_champion.lock().unwrap())
                    .ok_or(ActionError::NoChampion)?;
                let mut overlay =
                    self.inner.store.load_overlay(champion)?.unwrap_or_else(|| Overlay::new(champion, SettingsMap::default()));
                set_changes(&mut overlay.settings);
                overlay.updated = time::OffsetDateTime::now_utc();
                self.inner.store.save_overlay(&overlay)?;
                let name = self.inner.champions.name(champion).unwrap_or_else(|| format!("champion {champion}"));
                format!("Saved {} for {name} only", count(changes.len(), "change"))
            }
            DriftChoice::KeepHere => {
                set_changes(&mut base);
                format!("Kept {} on this account only", count(changes.len(), "change"))
            }
            DriftChoice::Revert => format!("Reverted {}", count(changes.len(), "change")),
        };
        tracing::info!(?choice, changes = changes.len(), "settled changed settings");

        // The account goes to its base. Riot's servers already hold whatever is on it
        // now, so this is a write whenever the two differ; when the changes were kept
        // and no overlay is on, they do not.
        let discards = matches!(choice, DriftChoice::SaveToChampion | DriftChoice::Revert);
        // Keeping changes writes nothing, so nothing else would record them in the log.
        if !discards && !changes.is_empty() {
            let mut record = Snapshot::new(&said, Some(&connection.puuid), expected.clone());
            record.changes = changes.clone();
            self.inner.store.save_snapshot(&record, KEEP_SNAPSHOTS)?;
        }
        let (_, after) = self.write_settings(&base, discards.then_some(said.as_str())).await?;
        self.update_account(|account| {
            account.baseline = Some(after);
            account.overlay = None;
        })?;
        *self.inner.reset.lock().unwrap() = false;
        Ok(said)
    }
}

/// Takes the changes to muted settings into `baseline`. Returns how many there were.
pub(super) fn keep_muted(baseline: &mut SettingsMap, expected: &SettingsMap, current: &SettingsMap, muted: &[String]) -> usize {
    let mut kept = 0;
    for change in user_changes(expected, current) {
        if let (true, Some(value)) = (muted.contains(&mute_id(&change)), &change.to) {
            baseline.set(&change.file, &change.section, &change.key, value);
            kept += 1;
        }
    }
    kept
}

/// How a setting is named in the list of muted ones.
pub fn mute_id(change: &Change) -> String {
    format!("{}/{}/{}", change.file, change.section, change.key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_to_muted_settings_are_kept_in_the_baseline() {
        let mut baseline = SettingsMap::default();
        baseline.set("Game.cfg", "HUD", "ShowFPS", "0");
        baseline.set("Input.ini", "GameEvents", "evtCastSpell1", "[q]");
        let expected = baseline.clone();
        let mut current = baseline.clone();
        current.set("Game.cfg", "HUD", "ShowFPS", "1");
        current.set("Input.ini", "GameEvents", "evtCastSpell1", "[a]");

        let muted = ["Game.cfg/HUD/ShowFPS".to_owned()];
        assert_eq!(keep_muted(&mut baseline, &expected, &current, &muted), 1);
        // The muted one is no longer a difference; the other still is.
        let left = user_changes(&baseline, &current);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].key, "evtCastSpell1");
    }

}

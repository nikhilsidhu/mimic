//! What the user can ask for: capture the current settings, apply a profile, undo.

use std::time::Duration;

use super::{Connection, Engine, Status};
use crate::lcu::LcuError;
use crate::profiles::{Profile, ProfileError, Snapshot};
use crate::settings::{overlay_between, PersistedSettings, SettingsMap};

/// Snapshots kept before the oldest are deleted.
const KEEP_SNAPSHOTS: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error("League is not running with an account logged in")]
    NotConnected,
    #[error("there is no profile {0:?}")]
    NoSuchProfile(String),
    #[error("there is no snapshot to restore")]
    NoSnapshot,
    #[error("could not read League's settings file: {0}")]
    ReadSettings(String),
    #[error(transparent)]
    Lcu(#[from] LcuError),
    #[error(transparent)]
    Store(#[from] ProfileError),
}

pub type Result<T> = std::result::Result<T, ActionError>;

/// How long the client gets to write the settings file before it is read back.
const SETTLE: Duration = Duration::from_millis(400);

/// The outcome of writing settings to the client.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Applied {
    pub changed: usize,
    /// Settings the client refused to take, by key name.
    pub stuck: Vec<String>,
}

impl Applied {
    /// A sentence for the notice popup. `verb` is "Applied" or "Restored".
    pub fn describe(&self, verb: &str) -> String {
        let settings = |count: usize| if count == 1 { "1 setting".to_owned() } else { format!("{count} settings") };
        match (self.changed, self.stuck.as_slice()) {
            (0, []) => "Already up to date".to_owned(),
            (changed, []) => format!("{verb} {}. They take effect next game.", settings(changed)),
            (changed, stuck) => {
                format!("{verb} {}. League refused {}: {}", settings(changed), stuck.len(), stuck.join(", "))
            }
        }
    }
}

impl Engine {
    /// Saves the logged-in account's current settings as a new profile.
    pub async fn save_current_as(&self, name: &str) -> Result<Profile> {
        let _guard = self.inner.action_lock.lock().await;
        let connection = self.connection()?;
        let settings = read_settings(&connection)?;

        let profile = Profile::new(name, settings.clone());
        self.inner.store.save_profile(&profile)?;
        self.remember(Some(&profile.id), settings)?;
        tracing::info!(id = %profile.id, name, keys = profile.settings.len(), "saved current settings as a profile");
        Ok(profile)
    }

    /// Applies a profile to the logged-in account, after snapshotting what is there.
    pub async fn apply_profile(&self, id: &str) -> Result<Applied> {
        let _guard = self.inner.action_lock.lock().await;
        let profile = self.inner.store.load_profile(id)?.ok_or_else(|| ActionError::NoSuchProfile(id.to_owned()))?;
        let reason = format!("before applying '{}'", profile.name);
        let applied = self.write(&profile.settings, &reason, Some(&profile.id)).await?;
        tracing::info!(id, name = %profile.name, changed = applied.changed, "applied profile");
        Ok(applied)
    }

    /// Puts back the settings from before the most recent apply. Undoing twice redoes,
    /// because the undo itself is snapshotted first.
    pub async fn restore_last_snapshot(&self) -> Result<Applied> {
        let _guard = self.inner.action_lock.lock().await;
        // Only this account's snapshots: another account's settings are not an undo.
        let puuid = self.connection()?.puuid;
        let snapshot = self
            .inner
            .store
            .list_snapshots()?
            .into_iter()
            .find(|snapshot| snapshot.puuid.as_deref() == Some(puuid.as_str()))
            .ok_or(ActionError::NoSnapshot)?;
        let applied = self.write(&snapshot.settings, "before restoring a snapshot", None).await?;
        tracing::info!(taken = %snapshot.taken, changed = applied.changed, "restored snapshot");
        Ok(applied)
    }

    pub fn profiles(&self) -> Result<Vec<Profile>> {
        Ok(self.inner.store.list_profiles()?)
    }

    pub fn active_profile(&self) -> Option<String> {
        self.inner.store.load_state().ok()?.active_profile
    }

    /// Snapshots the current settings, then writes whatever differs from `target`.
    /// Keys the target does not mention are left alone: accounts have different key
    /// sets, and a missing key is not a request to delete.
    async fn write(&self, target: &SettingsMap, reason: &str, profile_id: Option<&str>) -> Result<Applied> {
        let connection = self.connection()?;
        let before = read_settings(&connection)?;
        let changes = overlay_between(&before, target);
        // Layout the game moved on its own is not worth a write; it still goes along
        // whenever something real changes.
        if changes.is_only_volatile() {
            self.remember(profile_id, before)?;
            return Ok(Applied::default());
        }

        let snapshot = Snapshot::new(reason, Some(&connection.puuid), before.clone());
        self.inner.store.save_snapshot(&snapshot, KEEP_SNAPSHOTS)?;

        // The client can veto a value (a key it wants for something else, say), so what
        // actually landed is read back, and whatever is still off gets one more try.
        let mut after = before;
        let mut remaining = changes.clone();
        for _ in 0..2 {
            connection.client.apply_settings(&remaining).await?;
            tokio::time::sleep(SETTLE).await;
            after = read_settings(&connection)?;
            remaining = overlay_between(&after, target);
            if remaining.is_only_volatile() {
                break;
            }
        }

        let stuck: Vec<String> = remaining
            .iter()
            .filter(|(file, section, key, _)| !crate::settings::is_volatile(file, section, key))
            .map(|(_, _, key, _)| key.to_owned())
            .collect();
        if !stuck.is_empty() {
            tracing::warn!(?stuck, "the client did not accept some settings");
        }
        self.remember(profile_id, after)?;
        Ok(Applied { changed: changes.len() - stuck.len(), stuck })
    }

    fn remember(&self, profile_id: Option<&str>, applied: SettingsMap) -> Result<()> {
        let mut state = self.inner.store.load_state()?;
        state.active_profile = profile_id.map(str::to_owned);
        state.applied = Some(applied);
        self.inner.store.save_state(&state)?;
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(())
    }

    /// The live connection, only once an account is logged in and its settings are down.
    /// Before that the settings file still belongs to whoever was logged in last.
    fn connection(&self) -> Result<Connection> {
        if !matches!(*self.status.borrow(), Status::Connected { .. }) {
            return Err(ActionError::NotConnected);
        }
        self.inner.connection.lock().unwrap().clone().ok_or(ActionError::NotConnected)
    }
}

fn read_settings(connection: &Connection) -> Result<SettingsMap> {
    let path = connection.install.persisted_settings();
    let text = std::fs::read_to_string(&path).map_err(|err| ActionError::ReadSettings(format!("{}: {err}", path.display())))?;
    let persisted: PersistedSettings =
        serde_json::from_str(&text).map_err(|err| ActionError::ReadSettings(format!("{}: {err}", path.display())))?;
    Ok(SettingsMap::from(&persisted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_the_outcome_of_an_apply() {
        assert_eq!(Applied::default().describe("Applied"), "Already up to date");
        assert_eq!(
            Applied { changed: 96, stuck: vec![] }.describe("Applied"),
            "Applied 96 settings. They take effect next game."
        );
        assert_eq!(
            Applied { changed: 94, stuck: vec!["evtPushToTalk".into(), "evtSelectAlly4".into()] }.describe("Restored"),
            "Restored 94 settings. League refused 2: evtPushToTalk, evtSelectAlly4"
        );
    }
}

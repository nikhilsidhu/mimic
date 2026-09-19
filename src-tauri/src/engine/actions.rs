//! What the user can ask for: capture the current settings, apply a profile, undo.

use std::time::Duration;

use super::{Connection, Engine, Status};
use crate::lcu::{LcuError, Summoner};
use crate::profiles::{Account, Profile, ProfileError, Snapshot};
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

/// What asking for a profile led to.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Applied(Applied),
    /// Nobody was logged in; the named profile waits for the next login.
    Queued(String),
}

impl Outcome {
    pub fn describe(&self) -> String {
        match self {
            Outcome::Applied(applied) => applied.describe("Applied"),
            Outcome::Queued(name) => format!("'{name}' will be applied the next time you log into League"),
        }
    }
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
    /// With nobody logged in it is queued for the next login instead: settings only
    /// stick when written through a client that has the account open.
    pub async fn apply_profile(&self, id: &str) -> Result<Outcome> {
        let _guard = self.inner.action_lock.lock().await;
        let profile = self.inner.store.load_profile(id)?.ok_or_else(|| ActionError::NoSuchProfile(id.to_owned()))?;
        if self.connection().is_err() {
            let mut state = self.inner.store.load_state()?;
            state.pending_apply = Some(profile.id.clone());
            self.inner.store.save_state(&state)?;
            self.inner.changed.send_modify(|revision| *revision += 1);
            tracing::info!(id, name = %profile.name, "queued profile for the next login");
            return Ok(Outcome::Queued(profile.name));
        }
        Ok(Outcome::Applied(self.apply_now(&profile).await?))
    }

    async fn apply_now(&self, profile: &Profile) -> Result<Applied> {
        let reason = format!("before applying '{}'", profile.name);
        let applied = self.write(&profile.settings, &reason, Some(&profile.id)).await?;
        tracing::info!(id = %profile.id, name = %profile.name, changed = applied.changed, "applied profile");
        Ok(applied)
    }

    /// Runs when an account becomes ready: records it, then applies a queued profile or,
    /// if the account asked for it, its own profile.
    pub(super) async fn on_login(self, account: Summoner, phase: String) {
        let riot_id = format!("{}#{}", account.game_name, account.tag_line);
        let result: Result<Option<String>> = async {
            let _guard = self.inner.action_lock.lock().await;
            let mut accounts = self.inner.store.load_accounts()?;
            let entry = accounts.accounts.entry(account.puuid.clone()).or_insert_with(|| Account {
                display_name: riot_id.clone(),
                profile_id: None,
                auto_apply: false,
            });
            entry.display_name = riot_id.clone();
            // An account that is already on one of the profiles is recognised as such,
            // so that nobody has to apply a profile just to tell us what is there.
            if entry.profile_id.is_none() {
                let current = read_settings(&self.connection()?)?;
                let last_used = self.inner.store.load_state()?.active_profile;
                entry.profile_id =
                    matching_profile(&current, &self.inner.store.list_profiles()?, last_used.as_deref());
            }
            let (mapped, auto_apply) = (entry.profile_id.clone(), entry.auto_apply);
            self.inner.store.save_accounts(&accounts)?;

            let mut state = self.inner.store.load_state()?;
            let queued = state.pending_apply.take();
            let id = match at_login(queued.as_deref(), mapped.as_deref(), auto_apply, &phase) {
                AtLogin::Apply(id) => id.to_owned(),
                AtLogin::Nothing => return Ok(None),
                AtLogin::GameUnderWay => {
                    tracing::info!(%phase, "not applying at login: a game is under way");
                    return Ok(None);
                }
            };
            // Only now is the queue used up.
            if queued.is_some() {
                self.inner.store.save_state(&state)?;
            }
            let Some(profile) = self.inner.store.load_profile(&id)? else { return Ok(None) };
            let applied = self.apply_now(&profile).await?;
            // Nothing to say when the account was already up to date.
            Ok((applied != Applied::default()).then(|| {
                let refused = match applied.stuck.len() {
                    0 => String::new(),
                    count => format!(", League refused {count}"),
                };
                format!("Applied '{}' to {riot_id}: {} settings changed{refused}", profile.name, applied.changed)
            }))
        }
        .await;

        match result {
            Ok(Some(message)) => drop(self.inner.notices.send(message)),
            Ok(None) => {}
            Err(err) => {
                tracing::warn!("could not apply at login: {err}");
                let _ = self.inner.notices.send(format!("{riot_id}: could not apply your profile: {err}"));
            }
        }
        self.inner.changed.send_modify(|revision| *revision += 1);
    }

    /// Whether the logged-in account applies its profile by itself at login.
    pub fn auto_apply(&self) -> bool {
        self.account().is_some_and(|account| account.auto_apply)
    }

    pub fn set_auto_apply(&self, enabled: bool) -> Result<()> {
        let puuid = self.connection()?.puuid;
        let mut accounts = self.inner.store.load_accounts()?;
        if let Some(account) = accounts.accounts.get_mut(&puuid) {
            account.auto_apply = enabled;
            self.inner.store.save_accounts(&accounts)?;
            self.inner.changed.send_modify(|revision| *revision += 1);
        }
        Ok(())
    }

    /// The name of the profile waiting for the next login, if any.
    pub fn pending_profile(&self) -> Option<String> {
        let id = self.inner.store.load_state().ok()?.pending_apply?;
        Some(self.inner.store.load_profile(&id).ok()??.name)
    }

    fn account(&self) -> Option<Account> {
        let puuid = self.connection().ok()?.puuid;
        self.inner.store.load_accounts().ok()?.accounts.remove(&puuid)
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

    /// The profile the logged-in account is on: the one last applied to or saved from it.
    pub fn active_profile(&self) -> Option<String> {
        self.account()?.profile_id
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
        // Saturating: resolving a key conflict can make the client unbind a key that was
        // not part of `changes`, so more can be stuck than was asked for.
        Ok(Applied { changed: changes.len().saturating_sub(stuck.len()), stuck })
    }

    /// Records what is now on the account. A profile id also becomes the account's
    /// profile; an undo (`None`) leaves that alone.
    fn remember(&self, profile_id: Option<&str>, applied: SettingsMap) -> Result<()> {
        let mut state = self.inner.store.load_state()?;
        state.active_profile = profile_id.map(str::to_owned);
        state.applied = Some(applied);
        self.inner.store.save_state(&state)?;

        if let (Some(profile_id), Ok(connection)) = (profile_id, self.connection()) {
            let mut accounts = self.inner.store.load_accounts()?;
            let account = accounts.accounts.entry(connection.puuid).or_insert_with(|| Account {
                display_name: self.status.borrow().riot_id().unwrap_or_default(),
                profile_id: None,
                auto_apply: false,
            });
            account.profile_id = Some(profile_id.to_owned());
            self.inner.store.save_accounts(&accounts)?;
        }
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

#[derive(Debug, PartialEq)]
enum AtLogin<'a> {
    Apply(&'a str),
    Nothing,
    /// There is something to apply, but it has to wait for a later login.
    GameUnderWay,
}

/// What to do when an account becomes ready. A profile the user queued wins over the
/// account's own profile, which only counts if the account opted into auto-apply. A
/// game that is starting or running already has its settings, so nothing is applied
/// then and a queued profile stays queued.
fn at_login<'a>(queued: Option<&'a str>, mapped: Option<&'a str>, auto_apply: bool, phase: &str) -> AtLogin<'a> {
    let Some(id) = queued.or(mapped.filter(|_| auto_apply)) else { return AtLogin::Nothing };
    if matches!(phase, "ChampSelect" | "GameStart" | "InProgress" | "Reconnect") {
        return AtLogin::GameUnderWay;
    }
    AtLogin::Apply(id)
}

/// The profile `current` already matches, if any: applying it would change nothing but
/// window layout. Duplicates happen, so with several matches the one used last wins,
/// then the most recently updated.
fn matching_profile(current: &SettingsMap, profiles: &[Profile], last_used: Option<&str>) -> Option<String> {
    profiles
        .iter()
        .filter(|profile| !profile.settings.is_empty() && overlay_between(current, &profile.settings).is_only_volatile())
        .max_by_key(|profile| (Some(profile.id.as_str()) == last_used, profile.updated))
        .map(|profile| profile.id.clone())
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
    fn login_applies_a_queued_profile_or_an_opted_in_one() {
        assert_eq!(at_login(None, None, true, "None"), AtLogin::Nothing);
        // Having a profile is not consent to auto-apply.
        assert_eq!(at_login(None, Some("main"), false, "Lobby"), AtLogin::Nothing);
        assert_eq!(at_login(None, Some("main"), true, "Lobby"), AtLogin::Apply("main"));
        // What the user explicitly queued wins, with or without auto-apply.
        assert_eq!(at_login(Some("alt"), Some("main"), true, "None"), AtLogin::Apply("alt"));
        assert_eq!(at_login(Some("alt"), None, false, "None"), AtLogin::Apply("alt"));
    }

    #[test]
    fn login_waits_when_a_game_is_under_way() {
        for phase in ["ChampSelect", "GameStart", "InProgress", "Reconnect"] {
            assert_eq!(at_login(Some("alt"), None, false, phase), AtLogin::GameUnderWay, "{phase}");
        }
        assert_eq!(at_login(None, None, false, "InProgress"), AtLogin::Nothing);
        assert_eq!(at_login(Some("alt"), None, false, "EndOfGame"), AtLogin::Apply("alt"));
    }

    #[test]
    fn recognises_the_profile_an_account_is_already_on() {
        let mut current = SettingsMap::default();
        current.set("Input.ini", "GameEvents", "evtCastSpell1", "[q]");
        current.set("Game.cfg", "HUD", "DeathRecapNativeOffsetX", "0.0852");
        current.set("Game.cfg", "HUD", "OnlyOnThisAccount", "1");

        // Same binds; the layout differs and the account has an extra key. Still a match.
        let mut same = SettingsMap::default();
        same.set("Input.ini", "GameEvents", "evtCastSpell1", "[q]");
        same.set("Game.cfg", "HUD", "DeathRecapNativeOffsetX", "0.1355");
        let mut other = SettingsMap::default();
        other.set("Input.ini", "GameEvents", "evtCastSpell1", "[a]");

        let profiles = [
            Profile { id: "other".into(), ..Profile::new("Other", other) },
            Profile { id: "same".into(), ..Profile::new("Same", same) },
            Profile { id: "empty".into(), ..Profile::new("Empty", SettingsMap::default()) },
        ];
        assert_eq!(matching_profile(&current, &profiles, None), Some("same".into()));
        assert_eq!(matching_profile(&current, &profiles[..1], None), None);

        // Between identical profiles the one used last wins over the newer one.
        let newer = Profile { id: "copy".into(), ..Profile::new("Copy", profiles[1].settings.clone()) };
        let with_copy = [profiles[1].clone(), newer];
        assert_eq!(matching_profile(&current, &with_copy, None), Some("copy".into()));
        assert_eq!(matching_profile(&current, &with_copy, Some("same")), Some("same".into()));
        // Having been used last does not make a profile match.
        assert_eq!(matching_profile(&current, &profiles, Some("other")), Some("same".into()));
    }

    #[test]
    fn describes_a_queued_apply() {
        let outcome = Outcome::Queued("Main".into());
        assert_eq!(outcome.describe(), "'Main' will be applied the next time you log into League");
    }

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

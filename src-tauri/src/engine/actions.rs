//! What the user can ask for: capture the current settings, apply a profile, undo.

use std::time::Duration;

use super::{Connection, Engine, Status};
use crate::lcu::{LcuError, Summoner};
use crate::profiles::{Account, Overlay, Profile, ProfileError, Snapshot};
use crate::settings::{diff, is_volatile, merge, overlay_between, Change, PersistedSettings, SettingsMap};

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
    #[error("that snapshot belongs to another account")]
    OtherAccount,
    #[error("mimic has not seen that account")]
    NoSuchAccount,
    #[error("this account is not on a profile")]
    NoProfile,
    #[error("there is no champion to save this for")]
    NoChampion,
    #[error("mimic does not know this account's settings yet; apply or save a profile first")]
    NoBaseline,
    #[error("a champion's settings are on right now; try again after the game")]
    OverlayActive,
    #[error("nothing on this account differs from its base, so there is nothing to keep")]
    NothingChanged,
    #[error("that file cannot be imported: {0}")]
    InvalidImport(String),
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

/// How long after a game the settings are left alone before being compared. The game
/// writes them on exit and the client then reloads them.
const AFTER_GAME_SETTLE: Duration = Duration::from_secs(6);
/// How long after an account becomes ready its settings are left alone before being read.
const LOGIN_SETTLE: Duration = Duration::from_secs(5);

/// Something the engine wants the user to see without having been asked.
#[derive(Debug, Clone, PartialEq)]
pub enum Announcement {
    Notice(String),
    /// Settings changed; [`Engine::drift`] has the details.
    Drift,
}

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
        let reason = format!("applied '{}'", profile.name);
        let applied = self.write(&profile.settings, &reason, Some(&profile.id)).await?;
        tracing::info!(id = %profile.id, name = %profile.name, changed = applied.changed, "applied profile");
        Ok(applied)
    }

    /// Runs when an account becomes ready: records it, then applies a queued profile or,
    /// if the account asked for it, its own profile.
    pub(super) async fn on_login(self, account: Summoner, phase: String) {
        let riot_id = format!("{}#{}", account.game_name, account.tag_line);
        // The client keeps adjusting the settings for a moment after it reports ready
        // (it was seen flipping the push-to-talk key and back), and reading in between
        // looks like the user changed something.
        tokio::time::sleep(LOGIN_SETTLE).await;
        let result: Result<Option<String>> = async {
            let _guard = self.inner.action_lock.lock().await;
            let connection = self.connection()?;
            let mut current = read_settings(&connection)?;
            // Riot resets settings to defaults now and then, typically with a patch. It
            // shows up below as changes, and the prompt says what happened.
            let reset = connection.client.did_reset().await.unwrap_or(false);
            *self.inner.reset.lock().unwrap() = reset;
            if reset {
                tracing::info!("Riot reset this account's settings");
            }
            let profiles = self.inner.store.list_profiles()?;
            let mut accounts = self.inner.store.load_accounts()?;
            let entry =
                accounts.accounts.entry(account.puuid.clone()).or_insert_with(|| Account::new(riot_id.clone()));
            entry.display_name = riot_id.clone();
            entry.icon = Some(account.profile_icon_id);
            entry.level = Some(account.summoner_level);
            // mimic or the PC went down while a champion's settings were on. They come
            // off before anything else, or they would look like changes the user made.
            if entry.overlay.is_some() && !in_game(&phase) {
                if let Some(baseline) = entry.baseline.clone() {
                    tracing::info!(champion = entry.overlay, "taking off a champion's settings left on from last time");
                    current = self.write_settings(&baseline, None).await?.1;
                }
                entry.overlay = None;
            }
            // An account that is already on one of the profiles is recognised as such,
            // so that nobody has to apply a profile just to tell us what is there.
            if entry.profile_id.is_none() {
                let last_used = self.inner.store.load_state()?.active_profile;
                entry.profile_id = matching_profile(&current, &profiles, last_used.as_deref());
            }
            // Without a baseline, one can only be assumed if the account is exactly on
            // its profile; otherwise there is no telling what the user changed.
            if entry.baseline.is_none() {
                let on_profile = entry.profile_id.as_ref().and_then(|id| profiles.iter().find(|p| p.id == *id));
                if on_profile.is_some_and(|profile| overlay_between(&current, &profile.settings).is_only_volatile()) {
                    entry.baseline = Some(current.clone());
                }
            }
            // Muted settings are kept as they are, without asking.
            if let (Some(expected), Some(mut baseline)) = (self.expected(entry)?, entry.baseline.clone()) {
                if keep_muted(&mut baseline, &expected, &current, &self.muted()) > 0 {
                    entry.baseline = Some(baseline);
                }
            }
            let drifted = self.expected(entry)?.is_some_and(|expected| !self.asked_about(&expected, &current).is_empty());
            let (mapped, auto_apply) = (entry.profile_id.clone(), entry.auto_apply);
            self.inner.store.save_accounts(&accounts)?;

            let mut state = self.inner.store.load_state()?;
            state.last_account = Some(account.puuid.clone());
            self.inner.store.save_state(&state)?;
            let queued = state.pending_apply.take();
            let id = match at_login(queued.as_deref(), mapped.as_deref(), auto_apply, drifted, &phase) {
                AtLogin::Apply(id) => id.to_owned(),
                AtLogin::Nothing => return Ok(None),
                AtLogin::AskAboutChanges => {
                    tracing::info!("settings changed while mimic was not looking; asking before anything is applied");
                    let _ = self.inner.notices.send(Announcement::Drift);
                    return Ok(None);
                }
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
            Ok(Some(message)) => drop(self.inner.notices.send(Announcement::Notice(message))),
            Ok(None) => {}
            // Logged out again before the settings had settled: nothing to report.
            Err(ActionError::NotConnected) => tracing::debug!("the account went away during login"),
            Err(err) => {
                tracing::warn!("could not apply at login: {err}");
                let message = format!("{riot_id}: could not apply your profile: {err}");
                let _ = self.inner.notices.send(Announcement::Notice(message));
            }
        }
        self.inner.changed.send_modify(|revision| *revision += 1);
    }

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

    pub(super) fn account(&self) -> Option<Account> {
        let puuid = self.connection().ok()?.puuid;
        self.inner.store.load_accounts().ok()?.accounts.remove(&puuid)
    }

    /// Changes the logged-in account's record and tells the UI.
    pub(super) fn update_account(&self, change: impl FnOnce(&mut Account)) -> Result<()> {
        let puuid = self.connection()?.puuid;
        let mut accounts = self.inner.store.load_accounts()?;
        let account = accounts
            .accounts
            .entry(puuid)
            .or_insert_with(|| Account::new(self.status.borrow().riot_id().unwrap_or_default()));
        change(account);
        self.inner.store.save_accounts(&accounts)?;
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(())
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
        let applied = self.write(&snapshot.settings, "restored an earlier state", None).await?;
        tracing::info!(taken = %snapshot.taken, changed = applied.changed, "restored snapshot");
        Ok(applied)
    }

    /// Every snapshot, newest first.
    pub fn snapshots(&self) -> Result<Vec<Snapshot>> {
        Ok(self.inner.store.list_snapshots()?)
    }

    /// Puts back one particular snapshot, named by when it was taken (unix nanoseconds,
    /// as the UI got it). It has to belong to the logged-in account.
    pub async fn restore_snapshot(&self, taken: &str) -> Result<Applied> {
        let _guard = self.inner.action_lock.lock().await;
        let puuid = self.connection()?.puuid;
        let snapshot = self
            .inner
            .store
            .list_snapshots()?
            .into_iter()
            .find(|snapshot| snapshot_id(snapshot) == taken)
            .ok_or(ActionError::NoSnapshot)?;
        if snapshot.puuid.as_deref() != Some(puuid.as_str()) {
            return Err(ActionError::OtherAccount);
        }
        let applied = self.write(&snapshot.settings, "restored an earlier state", None).await?;
        tracing::info!(taken = %snapshot.taken, changed = applied.changed, "restored snapshot");
        Ok(applied)
    }

    /// The account last logged in, with its record.
    pub fn last_account(&self) -> Option<(String, Account)> {
        let puuid = self.inner.store.load_state().ok()?.last_account?;
        let account = self.inner.store.load_accounts().ok()?.accounts.remove(&puuid)?;
        Some((puuid, account))
    }

    /// Every account mimic has seen, with its record, keyed by puuid.
    pub fn accounts(&self) -> Result<Vec<(String, Account)>> {
        Ok(self.inner.store.load_accounts()?.accounts.into_iter().collect())
    }

    pub fn set_account_auto_apply(&self, puuid: &str, enabled: bool) -> Result<()> {
        let mut accounts = self.inner.store.load_accounts()?;
        let account = accounts.accounts.get_mut(puuid).ok_or(ActionError::NoSuchAccount)?;
        account.auto_apply = enabled;
        self.inner.store.save_accounts(&accounts)?;
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(())
    }

    /// Forgets an account: its profile link, auto-apply and baseline. Its settings on
    /// Riot's side are not touched. If it logs in again it starts fresh.
    pub fn forget_account(&self, puuid: &str) -> Result<()> {
        let mut accounts = self.inner.store.load_accounts()?;
        accounts.accounts.remove(puuid).ok_or(ActionError::NoSuchAccount)?;
        self.inner.store.save_accounts(&accounts)?;
        tracing::info!(puuid = %puuid.chars().take(8).collect::<String>(), "forgot account");
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(())
    }

    /// Overwrites a profile with what is on the logged-in account right now, and puts
    /// the account on it. Refused while a champion's settings are on, as those are not
    /// the account's own.
    pub async fn update_profile(&self, id: &str) -> Result<usize> {
        let _guard = self.inner.action_lock.lock().await;
        let connection = self.connection()?;
        if self.account().is_some_and(|account| account.overlay.is_some()) {
            return Err(ActionError::OverlayActive);
        }
        let mut profile =
            self.inner.store.load_profile(id)?.ok_or_else(|| ActionError::NoSuchProfile(id.to_owned()))?;
        let current = read_settings(&connection)?;
        let changes = user_changes(&profile.settings, &current);

        if !changes.is_empty() {
            // The account itself does not change, so this is the only record of it.
            let mut record =
                Snapshot::new(&format!("updated '{}'", profile.name), Some(&connection.puuid), current.clone());
            record.changes = changes.clone();
            self.inner.store.save_snapshot(&record, KEEP_SNAPSHOTS)?;
        }
        profile.settings = current.clone();
        profile.updated = time::OffsetDateTime::now_utc();
        self.inner.store.save_profile(&profile)?;
        self.remember(Some(&profile.id), current)?;
        tracing::info!(id, name = %profile.name, changed = changes.len(), "updated profile from the account");
        Ok(changes.len())
    }

    /// A copy of a profile under a free name.
    pub async fn duplicate_profile(&self, id: &str) -> Result<Profile> {
        let _guard = self.inner.action_lock.lock().await;
        let original = self.inner.store.load_profile(id)?.ok_or_else(|| ActionError::NoSuchProfile(id.to_owned()))?;
        let taken: Vec<String> = self.inner.store.list_profiles()?.into_iter().map(|profile| profile.name).collect();
        let base: String = original.name.chars().take(35).collect();
        let name = (1..)
            .map(|n| if n == 1 { format!("{base} copy") } else { format!("{base} copy {n}") })
            .find(|candidate| !taken.contains(candidate))
            .expect("an unbounded range always yields a free name");

        let mut copy = Profile::new(&name, original.settings);
        copy.client = original.client;
        self.inner.store.save_profile(&copy)?;
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(copy)
    }

    /// A profile's settings, and what applying it to the logged-in account would change
    /// (`None` when nobody is logged in).
    pub fn profile_details(&self, id: &str) -> Result<(Profile, Option<Vec<Change>>)> {
        let profile = self.inner.store.load_profile(id)?.ok_or_else(|| ActionError::NoSuchProfile(id.to_owned()))?;
        let preview = match self.connection() {
            Ok(connection) => Some(would_change(&read_settings(&connection)?, &profile.settings)),
            Err(_) => None,
        };
        Ok((profile, preview))
    }

    pub async fn rename_profile(&self, id: &str, name: &str) -> Result<()> {
        let _guard = self.inner.action_lock.lock().await;
        let mut profile =
            self.inner.store.load_profile(id)?.ok_or_else(|| ActionError::NoSuchProfile(id.to_owned()))?;
        profile.name = name.to_owned();
        self.inner.store.save_profile(&profile)?;
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(())
    }

    /// Deletes a profile and forgets it wherever it was referenced. Accounts that were
    /// on it keep their settings; they just no longer have a profile. Returns its name.
    pub async fn delete_profile(&self, id: &str) -> Result<String> {
        let _guard = self.inner.action_lock.lock().await;
        let profile = self.inner.store.load_profile(id)?.ok_or_else(|| ActionError::NoSuchProfile(id.to_owned()))?;
        self.inner.store.delete_profile(id)?;

        let mut accounts = self.inner.store.load_accounts()?;
        for account in accounts.accounts.values_mut().filter(|account| account.profile_id.as_deref() == Some(id)) {
            account.profile_id = None;
            account.auto_apply = false;
        }
        self.inner.store.save_accounts(&accounts)?;

        let mut state = self.inner.store.load_state()?;
        for reference in [&mut state.active_profile, &mut state.pending_apply] {
            if reference.as_deref() == Some(id) {
                *reference = None;
            }
        }
        self.inner.store.save_state(&state)?;

        tracing::info!(id, name = %profile.name, "deleted profile");
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(profile.name)
    }

    pub fn profiles(&self) -> Result<Vec<Profile>> {
        Ok(self.inner.store.list_profiles()?)
    }

    /// The profile the logged-in account is on: the one last applied to or saved from it.
    pub fn active_profile(&self) -> Option<String> {
        self.account()?.profile_id
    }

    /// Snapshots the current settings, writes whatever differs from `target`, and makes
    /// the result the account's baseline.
    async fn write(&self, target: &SettingsMap, reason: &str, profile_id: Option<&str>) -> Result<Applied> {
        let (applied, after) = self.write_settings(target, Some(reason)).await?;
        self.remember(profile_id, after)?;
        Ok(applied)
    }

    /// Writes whatever differs from `target` and returns what is on the account
    /// afterwards. Keys the target does not mention are left alone: accounts have
    /// different key sets, and a missing key is not a request to delete. A snapshot is
    /// taken first if a `snapshot` reason is given; overlays go without, or a few
    /// rerolls would push every real snapshot out.
    pub(super) async fn write_settings(
        &self,
        target: &SettingsMap,
        snapshot: Option<&str>,
    ) -> Result<(Applied, SettingsMap)> {
        let connection = self.connection()?;
        let before = read_settings(&connection)?;
        let changes = overlay_between(&before, target);
        // Layout the game moved on its own is not worth a write; it still goes along
        // whenever something real changes.
        if changes.is_only_volatile() {
            return Ok((Applied::default(), before));
        }

        // Saved before anything is written, for safety, and once more afterwards with what
        // the write ended up altering, which is what the change log shows.
        let mut record = match snapshot {
            Some(reason) => {
                let record = Snapshot::new(reason, Some(&connection.puuid), before.clone());
                self.inner.store.save_snapshot(&record, KEEP_SNAPSHOTS)?;
                Some(record)
            }
            None => None,
        };

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
            .filter(|(file, section, key, _)| !is_volatile(file, section, key))
            .map(|(_, _, key, _)| key.to_owned())
            .collect();
        if !stuck.is_empty() {
            tracing::warn!(?stuck, "the client did not accept some settings");
        }
        if let Some(record) = &mut record {
            record.changes = user_changes(&record.settings, &after);
            self.inner.store.save_snapshot(record, KEEP_SNAPSHOTS)?;
        }
        // Saturating: resolving a key conflict can make the client unbind a key that was
        // not part of `changes`, so more can be stuck than was asked for.
        Ok((Applied { changed: changes.len().saturating_sub(stuck.len()), stuck }, after))
    }
    /// Records what is now on the account as its baseline. A profile id also becomes the
    /// account's profile; an undo or a kept change (`None`) leaves that alone.
    fn remember(&self, profile_id: Option<&str>, on_account: SettingsMap) -> Result<()> {
        if let Some(profile_id) = profile_id {
            let mut state = self.inner.store.load_state()?;
            state.active_profile = Some(profile_id.to_owned());
            self.inner.store.save_state(&state)?;
        }

        if let Ok(connection) = self.connection() {
            let mut accounts = self.inner.store.load_accounts()?;
            let account = accounts
                .accounts
                .entry(connection.puuid)
                .or_insert_with(|| Account::new(self.status.borrow().riot_id().unwrap_or_default()));
            if let Some(profile_id) = profile_id {
                account.profile_id = Some(profile_id.to_owned());
            }
            // From here on, anything that differs from this is a change the user made.
            account.baseline = Some(on_account);
            self.inner.store.save_accounts(&accounts)?;
        }
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(())
    }

    /// The live connection, only once an account is logged in and its settings are down.
    /// Before that the settings file still belongs to whoever was logged in last.
    pub(super) fn connection(&self) -> Result<Connection> {
        if !matches!(*self.status.borrow(), Status::Connected { .. }) {
            return Err(ActionError::NotConnected);
        }
        self.inner.connection.lock().unwrap().clone().ok_or(ActionError::NotConnected)
    }
}

#[derive(Debug, PartialEq)]
enum AtLogin<'a> {
    Apply(&'a str),
    /// The account changed while mimic was not looking. Ask; do not overwrite.
    AskAboutChanges,
    Nothing,
    /// There is something to do, but it has to wait for a later login.
    GameUnderWay,
}

/// What to do when an account becomes ready. A profile the user queued wins, as an
/// explicit request. Otherwise settings that changed behind mimic's back are asked
/// about before auto-apply could overwrite them. The account's own profile only counts
/// if the account opted into auto-apply. A game that is starting or running already
/// has its settings, so nothing happens then and a queued profile stays queued.
fn at_login<'a>(
    queued: Option<&'a str>,
    mapped: Option<&'a str>,
    auto_apply: bool,
    drifted: bool,
    phase: &str,
) -> AtLogin<'a> {
    let wanted = match (queued, drifted, mapped.filter(|_| auto_apply)) {
        (Some(id), _, _) => AtLogin::Apply(id),
        (None, true, _) => AtLogin::AskAboutChanges,
        (None, false, Some(id)) => AtLogin::Apply(id),
        (None, false, None) => return AtLogin::Nothing,
    };
    if in_game(phase) {
        return AtLogin::GameUnderWay;
    }
    wanted
}

/// How the UI refers to a snapshot: when it was taken, in unix nanoseconds.
pub fn snapshot_id(snapshot: &Snapshot) -> String {
    snapshot.taken.unix_timestamp_nanos().to_string()
}

/// Phases in which the game is starting or running.
pub(super) fn in_game(phase: &str) -> bool {
    matches!(phase, "ChampSelect" | "GameStart" | "InProgress" | "Reconnect")
}

/// What differs between the baseline and what is on the account now, leaving out the
/// window layout the game rewrites on its own.
pub(super) fn user_changes(baseline: &SettingsMap, current: &SettingsMap) -> Vec<Change> {
    diff(baseline, current)
        .into_iter()
        .filter(|change| !is_volatile(&change.file, &change.section, &change.key))
        .collect()
}

/// Takes the changes to muted settings into `baseline`. Returns how many there were.
fn keep_muted(baseline: &mut SettingsMap, expected: &SettingsMap, current: &SettingsMap, muted: &[String]) -> usize {
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

/// What writing `target` onto `current` would alter: only keys the target has, as a
/// missing key is not a request to delete, and not the layout that moves on its own.
pub(super) fn would_change(current: &SettingsMap, target: &SettingsMap) -> Vec<Change> {
    user_changes(current, &merge(current, target))
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
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

pub(super) fn read_settings(connection: &Connection) -> Result<SettingsMap> {
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
        assert_eq!(at_login(None, None, true, false, "None"), AtLogin::Nothing);
        // Having a profile is not consent to auto-apply.
        assert_eq!(at_login(None, Some("main"), false, false, "Lobby"), AtLogin::Nothing);
        assert_eq!(at_login(None, Some("main"), true, false, "Lobby"), AtLogin::Apply("main"));
        // What the user explicitly queued wins, with or without auto-apply.
        assert_eq!(at_login(Some("alt"), Some("main"), true, false, "None"), AtLogin::Apply("alt"));
        assert_eq!(at_login(Some("alt"), None, false, false, "None"), AtLogin::Apply("alt"));
    }

    #[test]
    fn login_asks_before_auto_apply_overwrites_changes() {
        // Changed behind our back: auto-apply must not silently undo it.
        assert_eq!(at_login(None, Some("main"), true, true, "None"), AtLogin::AskAboutChanges);
        assert_eq!(at_login(None, Some("main"), false, true, "None"), AtLogin::AskAboutChanges);
        // An explicit request still goes ahead.
        assert_eq!(at_login(Some("alt"), Some("main"), true, true, "None"), AtLogin::Apply("alt"));
    }

    #[test]
    fn login_waits_when_a_game_is_under_way() {
        for phase in ["ChampSelect", "GameStart", "InProgress", "Reconnect"] {
            assert_eq!(at_login(Some("alt"), None, false, false, phase), AtLogin::GameUnderWay, "{phase}");
            assert_eq!(at_login(None, Some("main"), true, true, phase), AtLogin::GameUnderWay, "{phase}");
        }
        assert_eq!(at_login(None, None, false, false, "InProgress"), AtLogin::Nothing);
        assert_eq!(at_login(Some("alt"), None, false, false, "EndOfGame"), AtLogin::Apply("alt"));
    }

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

    #[test]
    fn a_preview_lists_only_what_an_apply_would_alter() {
        let mut current = SettingsMap::default();
        current.set("Input.ini", "GameEvents", "evtCastSpell1", "[q]");
        current.set("Input.ini", "GameEvents", "evtCastSpell2", "[w]");
        current.set("Game.cfg", "HUD", "OnlyOnThisAccount", "1");
        current.set("Game.cfg", "ItemShop", "CurrentTab", "0");

        let mut profile = SettingsMap::default();
        profile.set("Input.ini", "GameEvents", "evtCastSpell1", "[Shift][q]");
        profile.set("Input.ini", "GameEvents", "evtCastSpell2", "[w]");
        profile.set("Game.cfg", "ItemShop", "CurrentTab", "1");

        // One real difference. The key the profile lacks is not deleted, and the shop's
        // state does not count.
        let changes = would_change(&current, &profile);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "evtCastSpell1");
        assert_eq!((changes[0].from.as_deref(), changes[0].to.as_deref()), (Some("[q]"), Some("[Shift][q]")));
        assert!(would_change(&current, &current).is_empty());
    }

    #[test]
    fn user_changes_ignore_layout_the_game_moves() {
        let mut baseline = SettingsMap::default();
        baseline.set("Input.ini", "GameEvents", "evtCastSpell1", "[q]");
        baseline.set("Game.cfg", "HUD", "DeathRecapNativeOffsetX", "0.1355");
        let mut current = baseline.clone();
        current.set("Game.cfg", "HUD", "DeathRecapNativeOffsetX", "0.0852");
        assert!(user_changes(&baseline, &current).is_empty());

        current.set("Input.ini", "GameEvents", "evtCastSpell1", "[Shift][q]");
        let changes = user_changes(&baseline, &current);
        assert_eq!(changes.len(), 1);
        assert_eq!((changes[0].key.as_str(), changes[0].to.as_deref()), ("evtCastSpell1", Some("[Shift][q]")));
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

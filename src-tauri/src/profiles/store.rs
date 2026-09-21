use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use super::model::{Accounts, Overlay, Profile, Snapshot, State, SCHEMA_VERSION};
use super::{ProfileError, Result};

/// The data directory (`%APPDATA%\mimic` in the app):
///
/// ```text
/// profiles/<id>.json
/// overlays/<championId>.json
/// snapshots/<unix nanos>.json
/// accounts.json
/// ```
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Store { root: root.into() }
    }

    // Profiles

    pub fn save_profile(&self, profile: &Profile) -> Result<()> {
        write_json(&self.profile_path(&profile.id)?, profile)
    }

    pub fn load_profile(&self, id: &str) -> Result<Option<Profile>> {
        read_json(&self.profile_path(id)?)
    }

    pub fn delete_profile(&self, id: &str) -> Result<()> {
        remove(&self.profile_path(id)?)
    }

    /// Every profile, sorted by name.
    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        let mut profiles: Vec<Profile> = read_dir_json(&self.root.join("profiles"))?;
        profiles.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(profiles)
    }

    fn profile_path(&self, id: &str) -> Result<PathBuf> {
        // Ids end up in file names, and imported profiles bring their own.
        let valid = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid {
            return Err(ProfileError::InvalidId(id.to_owned()));
        }
        Ok(self.root.join("profiles").join(format!("{id}.json")))
    }

    // Overlays

    pub fn save_overlay(&self, overlay: &Overlay) -> Result<()> {
        write_json(&self.overlay_path(overlay.champion_id), overlay)
    }

    pub fn load_overlay(&self, champion_id: u32) -> Result<Option<Overlay>> {
        read_json(&self.overlay_path(champion_id))
    }

    pub fn delete_overlay(&self, champion_id: u32) -> Result<()> {
        remove(&self.overlay_path(champion_id))
    }

    /// Every overlay, sorted by champion id.
    pub fn list_overlays(&self) -> Result<Vec<Overlay>> {
        let mut overlays: Vec<Overlay> = read_dir_json(&self.root.join("overlays"))?;
        overlays.sort_by_key(|overlay| overlay.champion_id);
        Ok(overlays)
    }

    fn overlay_path(&self, champion_id: u32) -> PathBuf {
        self.root.join("overlays").join(format!("{champion_id}.json"))
    }

    // Accounts

    pub fn load_accounts(&self) -> Result<Accounts> {
        let accounts = read_json(&self.root.join("accounts.json"))?;
        Ok(accounts.unwrap_or(Accounts { schema: SCHEMA_VERSION, ..Accounts::default() }))
    }

    pub fn save_accounts(&self, accounts: &Accounts) -> Result<()> {
        write_json(&self.root.join("accounts.json"), accounts)
    }

    // State

    pub fn load_state(&self) -> Result<State> {
        let state = read_json(&self.root.join("state.json"))?;
        Ok(state.unwrap_or(State { schema: SCHEMA_VERSION, ..State::default() }))
    }

    pub fn save_state(&self, state: &State) -> Result<()> {
        write_json(&self.root.join("state.json"), state)
    }

    // Snapshots

    /// Saves `snapshot`, then deletes the oldest ones beyond `keep`.
    pub fn save_snapshot(&self, snapshot: &Snapshot, keep: usize) -> Result<()> {
        let dir = self.root.join("snapshots");
        // Zero-padded so that file names sort by time.
        write_json(&dir.join(format!("{:020}.json", snapshot.taken.unix_timestamp_nanos())), snapshot)?;

        let mut files = json_files(&dir)?;
        files.sort();
        let excess = files.len().saturating_sub(keep);
        for path in &files[..excess] {
            remove(path)?;
        }
        Ok(())
    }

    /// Takes a snapshot back, as when the change it was taken for never happened.
    pub fn delete_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
        let path = self.root.join("snapshots").join(format!("{:020}.json", snapshot.taken.unix_timestamp_nanos()));
        if path.exists() {
            remove(&path)?;
        }
        Ok(())
    }

    /// Every snapshot, newest first.
    pub fn list_snapshots(&self) -> Result<Vec<Snapshot>> {
        let mut snapshots: Vec<Snapshot> = read_dir_json(&self.root.join("snapshots"))?;
        snapshots.sort_by(|a, b| b.taken.cmp(&a.taken));
        Ok(snapshots)
    }
}

fn io_error(path: &Path) -> impl FnOnce(std::io::Error) -> ProfileError + '_ {
    move |source| ProfileError::Io { path: path.to_owned(), source }
}

/// Writes to a temporary file first so a crash never leaves a half-written file behind.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io_error(dir))?;
    }
    let json = serde_json::to_vec_pretty(value).map_err(|source| ProfileError::Json { path: path.to_owned(), source })?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(io_error(&tmp))?;
    fs::rename(&tmp, path).map_err(io_error(path))
}

/// `Ok(None)` when the file does not exist.
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(io_error(path)(err)),
    };
    let json_error = |source| ProfileError::Json { path: path.to_owned(), source };
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(json_error)?;
    if let Some(found) = value.get("schema").and_then(serde_json::Value::as_u64) {
        if found > SCHEMA_VERSION as u64 {
            return Err(ProfileError::UnsupportedSchema { path: path.to_owned(), found: found as u32 });
        }
    }
    serde_json::from_value(value).map(Some).map_err(json_error)
}

fn json_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(io_error(dir)(err)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.map_err(io_error(dir))?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            files.push(path);
        }
    }
    Ok(files)
}

fn read_dir_json<T: DeserializeOwned>(dir: &Path) -> Result<Vec<T>> {
    let mut values = Vec::new();
    for path in json_files(dir)? {
        if let Some(value) = read_json(&path)? {
            values.push(value);
        }
    }
    Ok(values)
}

fn remove(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() != ErrorKind::NotFound => Err(io_error(path)(err)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::Account;
    use crate::settings::SettingsMap;

    fn settings(cast1: &str) -> SettingsMap {
        let mut map = SettingsMap::default();
        map.set("Input.ini", "GameEvents", "evtCastSpell1", cast1);
        map
    }

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        (dir, store)
    }

    #[test]
    fn empty_store_reads_as_empty() {
        let (_dir, store) = store();
        assert!(store.list_profiles().unwrap().is_empty());
        assert!(store.list_overlays().unwrap().is_empty());
        assert!(store.list_snapshots().unwrap().is_empty());
        assert!(store.load_accounts().unwrap().accounts.is_empty());
        assert_eq!(store.load_profile("missing").unwrap(), None);
        store.delete_profile("missing").unwrap();
    }

    #[test]
    fn profiles_round_trip_and_list_by_name() {
        let (_dir, store) = store();
        let main = Profile { id: "main".into(), ..Profile::new("main", settings("[q]")) };
        let alt = Profile { id: "alt".into(), ..Profile::new("Alt", settings("[a]")) };
        store.save_profile(&main).unwrap();
        store.save_profile(&alt).unwrap();

        assert_eq!(store.load_profile("main").unwrap(), Some(main));
        let names: Vec<String> = store.list_profiles().unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Alt", "main"]);

        store.delete_profile("alt").unwrap();
        assert_eq!(store.list_profiles().unwrap().len(), 1);
    }

    #[test]
    fn rejects_ids_that_would_escape_the_profiles_dir() {
        let (_dir, store) = store();
        for id in ["", "../evil", "a/b", "a\\b", "c:evil"] {
            assert!(matches!(store.load_profile(id), Err(ProfileError::InvalidId(_))), "{id:?}");
        }
    }

    #[test]
    fn refuses_files_from_a_newer_schema() {
        let (dir, store) = store();
        let mut profile = Profile { id: "future".into(), ..Profile::new("Future", settings("[q]")) };
        profile.schema = SCHEMA_VERSION + 1;
        store.save_profile(&profile).unwrap();

        assert!(matches!(store.load_profile("future"), Err(ProfileError::UnsupportedSchema { found, .. }) if found == SCHEMA_VERSION + 1));
        assert!(!dir.path().join("profiles/future.json.tmp").exists());
    }

    #[test]
    fn overlays_and_accounts_round_trip() {
        let (_dir, store) = store();
        let yasuo = Overlay::new(157, settings("[Shift][q]"));
        store.save_overlay(&yasuo).unwrap();
        assert_eq!(store.load_overlay(157).unwrap(), Some(yasuo));
        assert_eq!(store.load_overlay(1).unwrap(), None);
        store.delete_overlay(157).unwrap();
        assert!(store.list_overlays().unwrap().is_empty());

        let mut accounts = store.load_accounts().unwrap();
        let account = Account {
            profile_id: Some("main".into()),
            auto_apply: true,
            baseline: Some(settings("[q]")),
            ..Account::new("Player#NA1".into())
        };
        accounts.accounts.insert("puuid-1".into(), account);
        store.save_accounts(&accounts).unwrap();
        assert_eq!(store.load_accounts().unwrap(), accounts);
    }

    #[test]
    fn snapshots_keep_only_the_newest() {
        let (_dir, store) = store();
        for n in 0..5 {
            let mut snapshot = Snapshot::new(&format!("apply {n}"), None, settings("[q]"));
            snapshot.taken += time::Duration::seconds(n);
            store.save_snapshot(&snapshot, 3).unwrap();
        }
        let reasons: Vec<String> = store.list_snapshots().unwrap().into_iter().map(|s| s.reason).collect();
        assert_eq!(reasons, ["apply 4", "apply 3", "apply 2"]);
    }

    #[test]
    fn a_snapshot_can_be_taken_back() {
        let (_dir, store) = store();
        let kept = Snapshot::new("applied main", None, settings("[q]"));
        let mut failed = Snapshot::new("an apply that failed", None, settings("[q]"));
        failed.taken += time::Duration::seconds(1);
        store.save_snapshot(&kept, 20).unwrap();
        store.save_snapshot(&failed, 20).unwrap();

        store.delete_snapshot(&failed).unwrap();
        // Taking back one that is already gone is not an error.
        store.delete_snapshot(&failed).unwrap();

        let reasons: Vec<String> = store.list_snapshots().unwrap().into_iter().map(|s| s.reason).collect();
        assert_eq!(reasons, ["applied main"]);
    }
}

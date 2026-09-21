use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::settings::{Change, SettingsMap};

/// Bumped when a file's shape changes in a way older builds cannot read.
pub const SCHEMA_VERSION: u32 = 1;

/// A full set of portable settings. The file is also the export format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    pub settings: SettingsMap,
    /// Whitelisted client preference categories, `category -> data`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub client: BTreeMap<String, serde_json::Value>,
}

impl Profile {
    pub fn new(name: &str, settings: SettingsMap) -> Self {
        let now = OffsetDateTime::now_utc();
        Profile {
            schema: SCHEMA_VERSION,
            id: format!("{:x}", now.unix_timestamp_nanos()),
            name: name.to_owned(),
            created: now,
            updated: now,
            settings,
            client: BTreeMap::new(),
        }
    }
}

/// Per-champion overrides, applied on top of whichever profile is active.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overlay {
    pub schema: u32,
    pub champion_id: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    /// Only the keys this champion overrides.
    pub settings: SettingsMap,
}

impl Overlay {
    pub fn new(champion_id: u32, settings: SettingsMap) -> Self {
        Overlay { schema: SCHEMA_VERSION, champion_id, updated: OffsetDateTime::now_utc(), settings }
    }
}

/// `accounts.json`: which profile each Riot account uses, keyed by puuid.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Accounts {
    pub schema: u32,
    pub accounts: BTreeMap<String, Account>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    /// `gameName#tagLine` as last seen, for display only.
    pub display_name: String,
    pub profile_id: Option<String>,
    /// Apply the profile automatically when this account logs in.
    #[serde(default)]
    pub auto_apply: bool,
    /// What was on the account when mimic last saved, applied or accepted its settings.
    /// Anything that differs from this later is a change the user made. It is what
    /// actually landed, not the profile, so a value the client refused never counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<SettingsMap>,
    /// The champion whose overlay is on top of the baseline right now. Set before a game
    /// and cleared when the base is restored after it, so that a crash in between still
    /// leads to a restore at the next login.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<u32>,
    /// Profile icon and level as last seen, so the account can be shown while League is
    /// closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
}

impl Account {
    pub fn new(display_name: String) -> Self {
        Account { display_name, profile_id: None, auto_apply: false, baseline: None, overlay: None, icon: None, level: None }
    }
}

/// `state.json`: what mimic last did, so it can pick up where it left off.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub schema: u32,
    /// The profile most recently applied or captured.
    pub active_profile: Option<String>,
    /// A profile chosen while no account was logged in; applied at the next login.
    #[serde(default)]
    pub pending_apply: Option<String>,
    /// The account last logged in, to show while League is closed.
    #[serde(default)]
    pub last_account: Option<String>,
    /// The League folder the user chose, for installs that detection does not find.
    #[serde(default)]
    pub install_path: Option<String>,
    /// Settings the user does not want to be asked about, as `file/section/key`. Changes
    /// to them are kept on the account without a prompt.
    #[serde(default)]
    pub muted: Vec<String>,
    /// Do not pop up notices about what mimic did unasked, such as applying at login.
    #[serde(default)]
    pub hide_notices: bool,
}

/// The settings as they were right before mimic changed them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub taken: OffsetDateTime,
    /// Why it was taken, e.g. "before applying 'Main'".
    pub reason: String,
    pub puuid: Option<String>,
    pub settings: SettingsMap,
    /// What the change this snapshot was taken for ended up altering, setting by setting.
    /// Together the snapshots are the change log.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<Change>,
}

impl Snapshot {
    pub fn new(reason: &str, puuid: Option<&str>, settings: SettingsMap) -> Self {
        Snapshot {
            schema: SCHEMA_VERSION,
            taken: OffsetDateTime::now_utc(),
            reason: reason.to_owned(),
            puuid: puuid.map(str::to_owned),
            settings,
            changes: Vec::new(),
        }
    }
}

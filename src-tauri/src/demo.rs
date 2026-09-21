//! A made-up account and made-up data, for taking screenshots that show what mimic does
//! without showing anybody's real account. Development builds only: start mimic with
//! `MIMIC_DEMO=1`. It uses its own data folder, talks to no League client and runs next
//! to an installed mimic.

use std::path::{Path, PathBuf};

use time::{Duration, OffsetDateTime};

use crate::engine::{ChampionRef, Drift};
use crate::lcu::Summoner;
use crate::profiles::{Account, Accounts, Overlay, Profile, Snapshot, State, Store, SCHEMA_VERSION};
use crate::settings::{Change, PersistedSettings, SettingsMap};

const PUUID: &str = "demo-account";
const AZIR: u32 = 268;
const LEE_SIN: u32 = 64;
const ORIANNA: u32 = 61;
const VIKTOR: u32 = 112;

pub fn enabled() -> bool {
    cfg!(debug_assertions) && std::env::var_os("MIMIC_DEMO").is_some()
}

/// Where the demo keeps its data: next to the real folder.
fn dir_beside(real_data_dir: &Path) -> PathBuf {
    real_data_dir.with_file_name("mimic-demo")
}

/// The account the demo pretends is logged in.
pub fn account() -> Summoner {
    let real_data_dir = crate::platform::data_dir().unwrap_or_default();
    Summoner {
        puuid: PUUID.to_owned(),
        game_name: "Hide on bush".to_owned(),
        tag_line: "KR1".to_owned(),
        profile_icon_id: any_profile_icon(&real_data_dir).unwrap_or(0),
        summoner_level: 712,
    }
}

pub const PHASE: &str = "ChampSelect";
pub const QUEUE: &str = "Ranked Solo/Duo";

/// What the prompt about changed settings shows.
pub fn drift() -> Drift {
    Drift {
        profile: Some("main".to_owned()),
        champion: Some(ChampionRef { id: AZIR, name: "Azir".to_owned() }),
        changes: vec![
            bind("evtPlayerAttackMove", Some("[a]"), "[x]"),
            bind("evtUseVisionItem", Some("[4]"), "[c]"),
            change("Game.cfg", "Performance", "ShowFPSAndLatency", Some("0"), "1"),
        ],
        reset: false,
    }
}

/// Fills the demo's data folder, next to the real one, and returns it. Icons still come from
/// the real cache: none ship with mimic, and the web view may only load from there.
pub fn prepare(real_data_dir: &Path) -> std::io::Result<PathBuf> {
    let dir = dir_beside(real_data_dir);
    // Started afresh every time, so that what is shown never depends on the last run.
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir)?;
    }
    seed(&Store::new(&dir), real_data_dir).map_err(std::io::Error::other)?;
    Ok(dir)
}

fn seed(store: &Store, real_data_dir: &Path) -> Result<(), crate::profiles::ProfileError> {
    let now = OffsetDateTime::now_utc();
    let main = base_settings();
    let mut aram = main.clone();
    aram.set("Game.cfg", "HUD", "MinimapScale", "1.6");
    aram.set("Input.ini", "GameEvents", "evtCastAvatarSpell1", "[f]");
    let mut jungle = main.clone();
    jungle.set("Input.ini", "GameEvents", "evtCastAvatarSpell1", "[f]");
    jungle.set("Input.ini", "GameEvents", "evtCameraLockToggle", "[y]");

    for (id, name, settings) in [("demo-main", "main", &main), ("demo-aram", "aram", &aram), ("demo-jungle", "jungle", &jungle)] {
        let mut profile = Profile::new(name, settings.clone());
        profile.id = id.to_owned();
        store.save_profile(&profile)?;
    }

    // Overrides players really make: a kiting champion attack-moves on a mouse button, drag-aimed
    // abilities go back to normal cast, shields get an easier self-cast, and the ward-hop gets a
    // key of its own.
    for (champion, keys) in [
        (AZIR, vec![("evtPlayerAttackMoveClick", "[Button 4]"), ("evtSmartCastWithIndicatorSpell2", "[w]"), ("evtSmartCastSpell2", "[<Unbound>]"), ("evtCameraLockToggle", "[y]"), ("evtChampionOnly", "[`]"), ("evtUseVisionItem", "[t]")]),
        (VIKTOR, vec![("evtCastSpell3", "[e]"), ("evtSmartCastSpell3", "[<Unbound>]")]),
        (ORIANNA, vec![("evtSelfCastSpell3", "[Button 5]"), ("evtChampionOnly", "[`]")]),
        (LEE_SIN, vec![("evtUseVisionItem", "[c]"), ("evtSmartPlusSelfCastSpell2", "[Button 4]")]),
    ] {
        let mut settings = SettingsMap::default();
        for (key, value) in keys {
            settings.set("Input.ini", "GameEvents", key, value);
        }
        store.save_overlay(&Overlay::new(champion, settings))?;
    }

    let mut accounts = Accounts { schema: SCHEMA_VERSION, ..Default::default() };
    let mut faker = Account::new("Hide on bush#KR1".to_owned());
    faker.profile_id = Some("demo-main".to_owned());
    faker.auto_apply = true;
    faker.baseline = Some(main.clone());
    faker.overlay = Some(AZIR);
    faker.icon = any_profile_icon(real_data_dir);
    faker.level = Some(712);
    accounts.accounts.insert(PUUID.to_owned(), faker);
    let mut second = Account::new("Unkillable Demon#KR1".to_owned());
    second.profile_id = Some("demo-aram".to_owned());
    accounts.accounts.insert("demo-second".to_owned(), second);
    store.save_accounts(&accounts)?;

    store.save_state(&State {
        schema: SCHEMA_VERSION,
        active_profile: Some("demo-main".to_owned()),
        last_account: Some(PUUID.to_owned()),
        muted: vec!["Game.cfg/Performance/ShowFPSAndLatency".to_owned()],
        ..Default::default()
    })?;

    let history = [
        (50, "applied 'main'", vec![bind("evtSmartCastSpell1", Some("[a]"), "[q]"), bind("evtCastAvatarSpell1", Some("[f]"), "[d]")]),
        (26, "Saved 1 change to 'main'", vec![bind("evtCameraLockToggle", Some("[y]"), "[Space]")]),
        (2, "Saved 2 changes for Azir only", vec![bind("evtPlayerAttackMoveClick", None, "[Button 4]"), bind("evtSmartCastWithIndicatorSpell2", None, "[w]")]),
    ];
    for (hours_ago, reason, changes) in history {
        let mut snapshot = Snapshot::new(reason, Some(PUUID), main.clone());
        snapshot.taken = now - Duration::hours(hours_ago);
        snapshot.changes = changes;
        store.save_snapshot(&snapshot, 20)?;
    }
    Ok(())
}

/// A real set of settings, the one the tests use, with the demo's keys on top. Read from the
/// source tree, which is where a development build runs from.
fn base_settings() -> SettingsMap {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/PersistedSettings.json");
    let mut settings = std::fs::read(fixture)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PersistedSettings>(&bytes).ok())
        .map(|persisted| SettingsMap::from(&persisted))
        .unwrap_or_default();
    // Quick cast on the spell keys, normal cast on Shift and self cast on Alt, as many
    // players have it.
    let binds = [
        ("evtSmartCastSpell1", "[q]"),
        ("evtSmartCastSpell2", "[w]"),
        ("evtSmartCastSpell3", "[e]"),
        ("evtSmartCastSpell4", "[r]"),
        ("evtCastSpell1", "[Shift][q]"),
        ("evtCastSpell2", "[Shift][w]"),
        ("evtCastSpell3", "[Shift][e]"),
        ("evtCastSpell4", "[Shift][r]"),
        ("evtSelfCastSpell1", "[Alt][q]"),
        ("evtSelfCastSpell2", "[Alt][w]"),
        ("evtSelfCastSpell3", "[Alt][e]"),
        ("evtSelfCastSpell4", "[Alt][r]"),
        ("evtSmartCastWithIndicatorSpell2", "[<Unbound>]"),
        ("evtSmartPlusSelfCastSpell2", "[<Unbound>]"),
        ("evtCastAvatarSpell1", "[d]"),
        ("evtCastAvatarSpell2", "[f]"),
        ("evtUseVisionItem", "[4]"),
        ("evtPlayerAttackMove", "[a]"),
        ("evtPlayerAttackMoveClick", "[<Unbound>]"),
        ("evtCameraLockToggle", "[Space]"),
        ("evtChampionOnly", "[<Unbound>]"),
    ];
    for (key, value) in binds {
        settings.set("Input.ini", "GameEvents", key, value);
    }
    settings.set("Game.cfg", "HUD", "MinimapScale", "1.2");
    settings.set("Game.cfg", "Performance", "ShowFPSAndLatency", "0");
    settings
}

fn bind(key: &str, from: Option<&str>, to: &str) -> Change {
    change("Input.ini", "GameEvents", key, from, to)
}

fn change(file: &str, section: &str, key: &str, from: Option<&str>, to: &str) -> Change {
    Change {
        file: file.to_owned(),
        section: section.to_owned(),
        key: key.to_owned(),
        from: from.map(str::to_owned),
        to: Some(to.to_owned()),
    }
}

/// Any profile icon that happens to be cached.
fn any_profile_icon(data_dir: &Path) -> Option<u32> {
    let icons = std::fs::read_dir(data_dir.join("cache").join("profile-icons")).ok()?;
    icons.flatten().find_map(|entry| entry.path().file_stem()?.to_str()?.parse().ok())
}


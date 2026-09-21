//! What the web UI can ask of the app. Each action answers with a sentence to show.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::engine::{self, Drift, DriftChoice, Engine, OverlaySource};
use crate::{platform, tray, updates};

/// Longest profile name the UI may create.
const MAX_NAME: usize = 40;

/// Everything the tray panel displays. Re-fetched on every `view-changed` event.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// The account's Riot ID, or why there is none.
    status: String,
    connected: bool,
    /// The connected account's profile icon and level.
    account: Option<AccountView>,
    phase: Option<String>,
    /// What the account is up to, for a badge, e.g. "Swiftplay - In game".
    activity: Option<String>,
    /// Whether this account applies its profile by itself at login.
    auto_apply: bool,
    /// The profile waiting for the next login, by name.
    pending: Option<String>,
    profiles: Vec<ProfileView>,
    /// How many settings the user changed and has not decided about yet.
    changed: usize,
    /// The champion whose settings are on top of this account's base right now.
    active_overlay: Option<ChampionView>,
    overlays: Vec<OverlayView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    /// `gameName#tagLine`.
    name: String,
    /// Absolute path of the cached profile icon, for the asset protocol.
    icon: String,
    level: u32,
}

/// A champion's own settings.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayView {
    champion: ChampionView,
    /// What it overrides.
    settings: Vec<OverrideRow>,
}

/// One setting a champion overrides.
#[derive(Debug, Serialize)]
pub struct OverrideRow {
    file: String,
    section: String,
    key: String,
    value: String,
    /// What it replaces: the value in the account's base, if that is known and differs.
    from: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    id: String,
    name: String,
    active: bool,
    settings: usize,
    /// How many settings applying it would change on the logged-in account; absent with nobody
    /// logged in.
    differs: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionView {
    id: u32,
    name: String,
    /// Absolute path of the cached icon, for the asset protocol.
    icon: String,
    /// The logged-in account's mastery points on it; 0 if never played.
    mastery: u64,
}

impl ChampionView {
    fn of(engine: &Engine, id: u32) -> Self {
        let champions = engine.champions();
        ChampionView {
            id,
            name: champions.name(id).unwrap_or_else(|| format!("Champion {id}")),
            icon: champions.icon_path(id).to_string_lossy().into_owned(),
            mastery: champions.mastery(id),
        }
    }
}

/// Every champion, by name. Empty until a League client has been seen once.
#[tauri::command]
pub fn champions(engine: State<Engine>) -> Vec<ChampionView> {
    let champions = engine.champions();
    champions
        .list()
        .into_iter()
        .map(|champion| ChampionView {
            icon: champions.icon_path(champion.id).to_string_lossy().into_owned(),
            mastery: champions.mastery(champion.id),
            id: champion.id,
            name: champion.name,
        })
        .collect()
}

/// The connected account, or else the one last seen, so the header keeps its face while
/// League is closed.
fn account_view(engine: &Engine, status: &crate::engine::Status) -> Option<AccountView> {
    let icon_path = |id: u32| engine.champions().profile_icon_path(id).to_string_lossy().into_owned();
    if let Some(account) = status.account() {
        return Some(AccountView {
            name: status.riot_id().unwrap_or_default(),
            icon: icon_path(account.profile_icon_id),
            level: account.summoner_level,
        });
    }
    let (_, last) = engine.last_account()?;
    Some(AccountView { name: last.display_name, icon: icon_path(last.icon?), level: last.level.unwrap_or(0) })
}

type Answer = Result<String, String>;

#[tauri::command]
pub fn view(engine: State<Engine>) -> View {
    let status = engine.status.borrow().clone();
    let active = engine.active_profile();
    let profiles = engine.profiles().unwrap_or_else(|err| {
        tracing::error!("could not list profiles: {err}");
        Vec::new()
    });
    let base = engine.base_settings();
    let differences = engine.differences(&profiles);
    View {
        status: status.label(),
        connected: status.riot_id().is_some(),
        account: account_view(&engine, &status),
        phase: status.phase().map(str::to_owned),
        activity: status.activity(),
        auto_apply: engine.auto_apply(),
        pending: engine.pending_profile(),
        profiles: profiles
            .into_iter()
            .enumerate()
            .map(|(index, profile)| ProfileView {
                active: active.as_deref() == Some(profile.id.as_str()),
                settings: profile.settings.len(),
                differs: differences.as_ref().map(|counts| counts[index]),
                id: profile.id,
                name: profile.name,
            })
            .collect(),
        changed: changes_to_review(&engine).map_or(0, |drift| drift.changes.len()),
        active_overlay: engine.active_overlay().map(|id| ChampionView::of(&engine, id)),
        overlays: engine
            .overlays()
            .unwrap_or_default()
            .into_iter()
            .map(|overlay| OverlayView {
                champion: ChampionView::of(&engine, overlay.champion_id),
                settings: overlay
                    .settings
                    .iter()
                    .map(|(file, section, key, value)| OverrideRow {
                        from: base
                            .as_ref()
                            .and_then(|base| base.get(file, section, key))
                            .filter(|from| *from != value)
                            .map(str::to_owned),
                        file: file.to_owned(),
                        section: section.to_owned(),
                        key: key.to_owned(),
                        value: value.to_owned(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[tauri::command]
pub async fn update_profile(engine: State<'_, Engine>, id: String) -> Answer {
    let changed = engine.update_profile(&id).await.map_err(|err| format!("Could not update: {err}"))?;
    Ok(match changed {
        0 => "Already matches this account".to_owned(),
        1 => "Updated with 1 changed setting".to_owned(),
        n => format!("Updated with {n} changed settings"),
    })
}

#[tauri::command]
pub async fn duplicate_profile(engine: State<'_, Engine>, id: String) -> Answer {
    let copy = engine.duplicate_profile(&id).await.map_err(|err| format!("Could not duplicate: {err}"))?;
    Ok(format!("Created '{}'", copy.name))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDetails {
    /// Every setting in the profile.
    settings: Vec<SettingRow>,
    /// What applying it to the logged-in account would alter; absent with nobody logged in.
    preview: Option<Vec<crate::settings::Change>>,
}

#[derive(Debug, Serialize)]
pub struct SettingRow {
    file: String,
    section: String,
    key: String,
    value: String,
}

impl SettingRow {
    fn of((file, section, key, value): (&str, &str, &str, &str)) -> Self {
        SettingRow { file: file.to_owned(), section: section.to_owned(), key: key.to_owned(), value: value.to_owned() }
    }
}

#[tauri::command]
pub fn profile_details(engine: State<Engine>, id: String) -> Result<ProfileDetails, String> {
    let (profile, preview) = engine.profile_details(&id).map_err(|err| err.to_string())?;
    let settings = profile
        .settings
        .iter()
        .map(SettingRow::of)
        .collect();
    Ok(ProfileDetails { settings, preview })
}

/// Asks where to save a profile and writes it there. `Ok(None)` when the user cancels.
#[tauri::command]
pub async fn export_profile(app: AppHandle, engine: State<'_, Engine>, id: String) -> Result<Option<String>, String> {
    let (file_name, json) = engine.export_profile(&id).map_err(|err| format!("Could not export: {err}"))?;
    let chosen = app
        .dialog()
        .file()
        .set_title("Export profile")
        .set_file_name(&file_name)
        .add_filter("mimic profile", &["json"])
        .blocking_save_file();
    let Some(path) = chosen.and_then(|path| path.into_path().ok()) else { return Ok(None) };
    std::fs::write(&path, json).map_err(|err| format!("Could not write {}: {err}", path.display()))?;
    Ok(Some(format!("Exported to {}", path.display())))
}

/// Asks for an exported profile and adds it. `Ok(None)` when the user cancels.
#[tauri::command]
pub async fn import_profile(app: AppHandle, engine: State<'_, Engine>) -> Result<Option<String>, String> {
    let chosen = app.dialog().file().set_title("Import profile").add_filter("mimic profile", &["json"]).blocking_pick_file();
    let Some(path) = chosen.and_then(|path| path.into_path().ok()) else { return Ok(None) };
    let bytes = std::fs::read(&path).map_err(|err| format!("Could not read {}: {err}", path.display()))?;
    let profile = engine.import_profile(&bytes).await.map_err(|err| format!("Could not import: {err}"))?;
    Ok(Some(format!("Imported '{}' with {} settings", profile.name, profile.settings.len())))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    /// Opaque id for `restore_snapshot`.
    id: String,
    /// When it was taken, RFC 3339.
    taken: String,
    reason: String,
    /// The account it was taken on, as `gameName#tagLine` if known.
    account: Option<String>,
    /// Whether it belongs to the logged-in account and can be restored right now.
    restorable: bool,
    settings: usize,
    /// What the change it was taken for altered, setting by setting.
    changes: Vec<crate::settings::Change>,
}

/// Every snapshot, newest first.
#[tauri::command]
pub fn snapshots(engine: State<Engine>) -> Vec<SnapshotView> {
    let accounts = engine.accounts().unwrap_or_default();
    let connected = engine.status.borrow().account().map(|account| account.puuid.clone());
    engine
        .snapshots()
        .unwrap_or_default()
        .into_iter()
        .map(|snapshot| SnapshotView {
            id: engine::snapshot_id(&snapshot),
            taken: snapshot.taken.format(&time::format_description::well_known::Rfc3339).unwrap_or_default(),
            account: snapshot.puuid.as_ref().and_then(|puuid| {
                accounts.iter().find(|(id, _)| id == puuid).map(|(_, account)| account.display_name.clone())
            }),
            restorable: snapshot.puuid.is_some() && snapshot.puuid == connected,
            settings: snapshot.settings.len(),
            changes: snapshot.changes,
            reason: snapshot.reason,
        })
        .collect()
}

#[tauri::command]
pub async fn restore_snapshot(engine: State<'_, Engine>, id: String) -> Answer {
    let applied = engine.restore_snapshot(&id).await.map_err(|err| format!("Could not restore: {err}"))?;
    Ok(applied.describe("Restored"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRow {
    puuid: String,
    name: String,
    /// The profile it is on, by name.
    profile: Option<String>,
    auto_apply: bool,
    /// Whether this is the account logged in right now.
    connected: bool,
}

/// Every account mimic has seen, the connected one first.
#[tauri::command]
pub fn accounts(engine: State<Engine>) -> Vec<AccountRow> {
    let connected = engine.status.borrow().account().map(|account| account.puuid.clone());
    let profiles = engine.profiles().unwrap_or_default();
    let mut rows: Vec<AccountRow> = engine
        .accounts()
        .unwrap_or_default()
        .into_iter()
        .map(|(puuid, account)| AccountRow {
            connected: Some(&puuid) == connected.as_ref(),
            profile: account
                .profile_id
                .as_ref()
                .and_then(|id| profiles.iter().find(|profile| &profile.id == id))
                .map(|profile| profile.name.clone()),
            puuid,
            name: account.display_name,
            auto_apply: account.auto_apply,
        })
        .collect();
    rows.sort_by_key(|row| (!row.connected, row.name.to_lowercase()));
    rows
}

#[tauri::command]
pub fn set_account_auto_apply(engine: State<Engine>, puuid: String, enabled: bool) -> Result<(), String> {
    engine.set_account_auto_apply(&puuid, enabled).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn forget_account(engine: State<Engine>, puuid: String) -> Answer {
    engine.forget_account(&puuid).map_err(|err| format!("Could not forget: {err}"))?;
    Ok("Forgotten".to_owned())
}

/// Where a champion's settings could be taken from right now.
#[tauri::command]
pub fn overlay_sources(engine: State<Engine>) -> Result<Vec<OverlaySource>, String> {
    engine.overlay_sources().map_err(|err| err.to_string())
}

/// Gives a champion its own settings, from what changed just now or from a profile.
#[tauri::command]
pub async fn save_overlay(engine: State<'_, Engine>, champion: u32, profile: Option<String>) -> Answer {
    engine.save_overlay(champion, profile.as_deref()).await.map_err(|err| format!("Could not save: {err}"))
}

#[tauri::command]
pub async fn delete_overlay(engine: State<'_, Engine>, champion: u32) -> Answer {
    engine.delete_overlay(champion).await.map_err(|err| format!("Could not delete: {err}"))?;
    Ok("Deleted".to_owned())
}

/// Takes one setting out of a champion's own settings.
#[tauri::command]
pub async fn remove_overlay_setting(
    engine: State<'_, Engine>,
    champion: u32,
    file: String,
    section: String,
    key: String,
) -> Answer {
    engine
        .remove_overlay_setting(champion, &file, &section, &key)
        .await
        .map_err(|err| format!("Could not remove that: {err}"))?;
    Ok("Removed".to_owned())
}

#[tauri::command]
pub async fn apply_profile(engine: State<'_, Engine>, id: String) -> Answer {
    let outcome = engine.apply_profile(&id).await.map_err(|err| format!("Could not apply: {err}"))?;
    Ok(outcome.describe())
}

#[tauri::command]
pub fn set_auto_apply(engine: State<Engine>, enabled: bool) -> Result<(), String> {
    engine.set_auto_apply(enabled).map_err(|err| err.to_string())
}

fn valid_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(format!("A profile name needs 1 to {MAX_NAME} characters"));
    }
    Ok(name)
}

#[tauri::command]
pub async fn rename_profile(engine: State<'_, Engine>, id: String, name: String) -> Answer {
    let name = valid_name(&name)?;
    engine.rename_profile(&id, name).await.map_err(|err| format!("Could not rename: {err}"))?;
    Ok(format!("Renamed to '{name}'"))
}

#[tauri::command]
pub async fn delete_profile(engine: State<'_, Engine>, id: String) -> Answer {
    let name = engine.delete_profile(&id).await.map_err(|err| format!("Could not delete: {err}"))?;
    Ok(format!("Deleted '{name}'"))
}

#[tauri::command]
pub async fn save_current(engine: State<'_, Engine>, name: String) -> Answer {
    let name = valid_name(&name)?;
    let profile = engine.save_current_as(name).await.map_err(|err| format!("Could not save: {err}"))?;
    Ok(format!("Saved {} settings as '{}'", profile.settings.len(), profile.name))
}

#[tauri::command]
pub async fn undo_last(engine: State<'_, Engine>) -> Answer {
    let applied = engine.restore_last_snapshot().await.map_err(|err| format!("Could not undo: {err}"))?;
    Ok(applied.describe("Restored"))
}

/// The settings the user changed on this account and has not decided about, if any.
fn changes_to_review(engine: &Engine) -> Option<Drift> {
    // The demo's changes are made up, but muting one takes it off the list as it really would.
    if crate::demo::enabled() {
        let muted = engine.muted();
        let mut drift = crate::demo::drift();
        drift.changes.retain(|change| !muted.contains(&engine::mute_id(change)));
        return (!drift.changes.is_empty()).then_some(drift);
    }
    engine.drift().unwrap_or_else(|err| {
        tracing::warn!("could not check for changed settings: {err}");
        None
    })
}

#[tauri::command]
pub fn drift(engine: State<Engine>) -> Option<Drift> {
    changes_to_review(&engine)
}

/// The prompt about changed settings asks to be as tall as what it shows.
#[tauri::command]
pub fn fit_prompt(app: AppHandle, height: f64) {
    if let Err(err) = tray::fit_drift_prompt(&app, height) {
        tracing::debug!("could not resize the prompt: {err}");
    }
}

#[tauri::command]
pub async fn resolve_drift(engine: State<'_, Engine>, choice: DriftChoice) -> Answer {
    // The demo has no client to write to.
    if crate::demo::enabled() {
        return Ok("Nothing is saved in the demo".to_owned());
    }
    engine.resolve_drift(choice).await.map_err(|err| format!("Could not do that: {err}"))
}

/// The settings mimic does not ask about, as `file/section/key`.
#[tauri::command]
pub fn muted_settings(engine: State<Engine>) -> Vec<String> {
    engine.muted()
}

#[tauri::command]
pub async fn mute_setting(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    // The demo has no account whose settings could be kept.
    if crate::demo::enabled() {
        return engine.mute(&id).map_err(|err| err.to_string());
    }
    engine.mute_setting(&id).await.map_err(|err| format!("Could not mute that: {err}"))
}

#[tauri::command]
pub fn unmute_setting(engine: State<Engine>, id: String) -> Result<(), String> {
    engine.unmute(&id).map_err(|err| format!("Could not unmute that: {err}"))
}

#[tauri::command]
pub fn copy_riot_id(app: AppHandle, engine: State<Engine>) -> Answer {
    // The account shown: the connected one, or the last seen while League is closed.
    let status = engine.status.borrow().clone();
    let riot_id = account_view(&engine, &status).map(|account| account.name).ok_or("No account to copy")?;
    app.clipboard().write_text(riot_id.clone()).map_err(|err| format!("Could not copy: {err}"))?;
    Ok(format!("Copied {riot_id}"))
}

/// The League folder in use, if one has been found.
#[tauri::command]
pub fn install_path(engine: State<Engine>) -> Option<String> {
    engine.install_path().map(|path| path.to_string_lossy().into_owned())
}

/// Asks for the League folder and uses it. `Ok(None)` when the user cancels.
#[tauri::command]
pub async fn choose_install(app: AppHandle, engine: State<'_, Engine>) -> Result<Option<String>, String> {
    let chosen = app.dialog().file().set_title("Choose the League of Legends folder").blocking_pick_folder();
    let Some(folder) = chosen.and_then(|path| path.into_path().ok()) else { return Ok(None) };
    engine.choose_install(&folder)?;
    Ok(Some(format!("Using {}", folder.display())))
}

/// Whether mimic starts with Windows.
#[tauri::command]
pub fn autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let result = if enabled { autolaunch.enable() } else { autolaunch.disable() };
    result.map_err(|err| format!("Could not change startup: {err}"))
}

#[tauri::command]
pub fn open_manager(app: AppHandle) {
    tray::show_manager(&app);
}

#[derive(Debug, Serialize)]
pub struct UpdateStatus {
    /// The version running.
    current: String,
    /// A newer one, if one was found.
    available: Option<String>,
}

#[tauri::command]
pub fn update_status(app: AppHandle, available: State<updates::Available>) -> UpdateStatus {
    UpdateStatus { current: app.package_info().version.to_string(), available: available.version() }
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Answer {
    // The reason is for the log; it is of no use to the user.
    let found = updates::check(&app).await.map_err(|err| {
        tracing::warn!("could not check for updates: {err}");
        "Could not check for updates. Try again later.".to_owned()
    })?;
    match found {
        Some(version) => Ok(format!("mimic {version} is available")),
        None => Ok("mimic is up to date".to_owned()),
    }
}

/// Installs the newer version, which restarts mimic. Not while a game is under way: a
/// champion's settings may be on and have to come off after it.
#[tauri::command]
pub async fn install_update(app: AppHandle, engine: State<'_, Engine>) -> Result<(), String> {
    if engine.in_game() {
        return Err("Finish your game first; mimic restarts to update.".to_owned());
    }
    updates::install(&app).await.map_err(|err| {
        tracing::warn!("could not install the update: {err}");
        "Could not update. Try again later.".to_owned()
    })
}

/// Opens the manager with its champion picker showing.
#[tauri::command]
pub fn add_champion(app: AppHandle) {
    tray::show_manager(&app);
    let _ = app.emit("add-champion", ());
}

/// Whether what mimic does unasked is announced in a popup.
#[tauri::command]
pub fn shows_notices(engine: State<Engine>) -> bool {
    engine.shows_notices()
}

#[tauri::command]
pub fn set_shows_notices(engine: State<Engine>, enabled: bool) -> Result<(), String> {
    engine.set_shows_notices(enabled).map_err(|err| err.to_string())
}

/// Opens the folder with everything mimic stores: profiles, snapshots and logs.
#[tauri::command]
pub fn open_data_folder(app: AppHandle) -> Result<(), String> {
    let data = platform::data_dir().ok_or("APPDATA is not set")?;
    std::fs::create_dir_all(&data).map_err(|err| err.to_string())?;
    app.opener().open_path(data.to_string_lossy(), None::<&str>).map_err(|err| err.to_string())
}

/// Shows `message` in the notice popup. For windows that close before their action
/// has anything to say, like the changed-settings prompt.
#[tauri::command]
pub fn notify(app: AppHandle, message: String) {
    tray::show_notice(&app, message);
}

/// Closes the tray panel.
#[tauri::command]
pub fn dismiss(app: AppHandle) {
    let _ = tray::hide_panel(&app);
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

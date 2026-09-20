//! What the web UI can ask of the app. Each action answers with a sentence to show.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

use crate::engine::{Drift, DriftChoice, Engine, OverlaySource};
use crate::{platform, tray};

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
    /// The champion whose settings are on top of this account's base right now.
    active_overlay: Option<ChampionView>,
    overlays: Vec<OverlayView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    /// Absolute path of the cached profile icon, for the asset protocol.
    icon: String,
    level: u32,
}

/// A champion's own settings.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayView {
    champion: ChampionView,
    /// What it overrides, as `key` and `value`, for display.
    settings: Vec<(String, String)>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    id: String,
    name: String,
    active: bool,
    settings: usize,
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

type Answer = Result<String, String>;

#[tauri::command]
pub fn view(engine: State<Engine>) -> View {
    let status = engine.status.borrow().clone();
    let active = engine.active_profile();
    let profiles = engine.profiles().unwrap_or_else(|err| {
        tracing::error!("could not list profiles: {err}");
        Vec::new()
    });
    View {
        status: status.label(),
        connected: status.riot_id().is_some(),
        account: status.account().map(|account| AccountView {
            icon: engine.champions().profile_icon_path(account.profile_icon_id).to_string_lossy().into_owned(),
            level: account.summoner_level,
        }),
        phase: status.phase().map(str::to_owned),
        activity: status.activity(),
        auto_apply: engine.auto_apply(),
        pending: engine.pending_profile(),
        profiles: profiles
            .into_iter()
            .map(|profile| ProfileView {
                active: active.as_deref() == Some(profile.id.as_str()),
                settings: profile.settings.len(),
                id: profile.id,
                name: profile.name,
            })
            .collect(),
        active_overlay: engine.active_overlay().map(|id| ChampionView::of(&engine, id)),
        overlays: engine
            .overlays()
            .unwrap_or_default()
            .into_iter()
            .map(|overlay| OverlayView {
                champion: ChampionView::of(&engine, overlay.champion_id),
                settings: overlay.settings.iter().map(|(_, _, key, value)| (key.to_owned(), value.to_owned())).collect(),
            })
            .collect(),
    }
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

/// The settings the user changed on this account, if any.
#[tauri::command]
pub fn drift(engine: State<Engine>) -> Option<Drift> {
    engine.drift().unwrap_or_else(|err| {
        tracing::warn!("could not check for changed settings: {err}");
        None
    })
}

#[tauri::command]
pub async fn resolve_drift(engine: State<'_, Engine>, choice: DriftChoice) -> Answer {
    engine.resolve_drift(choice).await.map_err(|err| format!("Could not do that: {err}"))
}

#[tauri::command]
pub fn copy_riot_id(app: AppHandle, engine: State<Engine>) -> Answer {
    let riot_id = engine.status.borrow().riot_id().ok_or("No account is connected")?;
    app.clipboard().write_text(riot_id.clone()).map_err(|err| format!("Could not copy: {err}"))?;
    Ok(format!("Copied {riot_id}"))
}

#[tauri::command]
pub fn open_manager(app: AppHandle) {
    tray::show_manager(&app);
}

#[tauri::command]
pub fn open_logs(app: AppHandle) -> Result<(), String> {
    let logs = platform::data_dir().ok_or("APPDATA is not set")?.join("logs");
    std::fs::create_dir_all(&logs).map_err(|err| err.to_string())?;
    app.opener().open_path(logs.to_string_lossy(), None::<&str>).map_err(|err| err.to_string())
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

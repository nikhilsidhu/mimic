//! What the web UI can ask of the app. Each action answers with a sentence to show.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

use crate::engine::Engine;
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
    phase: Option<String>,
    /// Whether this account applies its profile by itself at login.
    auto_apply: bool,
    /// The profile waiting for the next login, by name.
    pending: Option<String>,
    profiles: Vec<ProfileView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    id: String,
    name: String,
    active: bool,
    settings: usize,
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
        phase: status.phase().map(str::to_owned),
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
    }
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

/// Shows `message` in the notice popup. The menu closes before its action finishes, so
/// this is how it reports the result.
#[tauri::command]
pub fn notify(app: AppHandle, message: String) {
    tray::show_notice(&app, message);
}

/// Closes the tray flyout that called it.
#[tauri::command]
pub fn dismiss(app: AppHandle, window: tauri::WebviewWindow) {
    if let Some(flyout) = tray::Flyout::from_label(window.label()) {
        let _ = tray::hide_flyout(&app, flyout);
    }
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

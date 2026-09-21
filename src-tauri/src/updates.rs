//! Looks for a newer mimic among the GitHub releases and installs it when asked.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// How long after start the first look happens, and how long between looks after that.
const FIRST_CHECK: Duration = Duration::from_secs(30);
const BETWEEN_CHECKS: Duration = Duration::from_secs(6 * 60 * 60);

/// The newer version last found, if any.
#[derive(Default)]
pub struct Available(Mutex<Option<String>>);

impl Available {
    pub fn version(&self) -> Option<String> {
        self.0.lock().unwrap().clone()
    }
}

/// Looks for a newer version and remembers the answer.
pub async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    let update = app.updater().map_err(|err| err.to_string())?.check().await.map_err(|err| err.to_string())?;
    let version = update.map(|update| update.version);
    *app.state::<Available>().0.lock().unwrap() = version.clone();
    let _ = app.emit("view-changed", ());
    Ok(version)
}

/// Downloads and runs the newer installer, which closes mimic and starts it again.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|err| err.to_string())?;
    let update = updater.check().await.map_err(|err| err.to_string())?.ok_or("mimic is up to date")?;
    tracing::info!(version = %update.version, "installing an update");
    update.download_and_install(|_, _| {}, || {}).await.map_err(|err| err.to_string())?;
    app.restart()
}

/// Looks for updates in the background for as long as mimic runs. Nothing is installed
/// until the user asks.
pub fn watch(app: AppHandle) {
    // A development build is not what the releases replace.
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            match check(&app).await {
                Ok(Some(version)) => tracing::info!(%version, "an update is available"),
                Ok(None) => tracing::debug!("no update available"),
                Err(err) => tracing::debug!("could not look for updates: {err}"),
            }
            tokio::time::sleep(BETWEEN_CHECKS).await;
        }
    });
}

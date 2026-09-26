//! Looks for a newer mimic among the GitHub releases and installs it when asked.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// How long after start the first look happens, and how long between looks after that.
const FIRST_CHECK: Duration = Duration::from_secs(30);
const BETWEEN_CHECKS: Duration = Duration::from_secs(6 * 60 * 60);

/// The update last found, if any: kept whole, so that installing it installs exactly what was
/// found. Checking again at that point is what failed: for a few minutes after a release GitHub
/// can still send some requests for the latest release to the one before, and a second check
/// then found nothing.
#[derive(Default)]
pub struct Available(Mutex<Option<Update>>);

impl Available {
    pub fn version(&self) -> Option<String> {
        self.0.lock().unwrap().as_ref().map(|update| update.version.clone())
    }
}

/// Looks for a newer version and remembers the answer. A check that finds nothing does not
/// forget an update found earlier, for the same reason.
pub async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    let found = app.updater().map_err(|err| err.to_string())?.check().await.map_err(|err| err.to_string())?;
    let available = app.state::<Available>();
    if let Some(update) = found {
        tracing::info!(version = %update.version, "an update is available");
        *available.0.lock().unwrap() = Some(update);
    } else {
        tracing::info!("no newer version found");
    }
    let _ = app.emit("view-changed", ());
    Ok(available.version())
}

/// Downloads and runs the update found, which closes mimic and starts it again. Checks first
/// only if none was found yet.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let known = app.state::<Available>().0.lock().unwrap().clone();
    let update = match known {
        Some(update) => update,
        None => app.updater().map_err(|err| err.to_string())?.check().await.map_err(|err| err.to_string())?.ok_or("mimic is up to date")?,
    };
    tracing::info!(version = %update.version, "installing an update");
    update.download_and_install(|_, _| {}, || {}).await.map_err(|err| {
        tracing::warn!(version = %update.version, "the update did not install: {err}");
        err.to_string()
    })?;
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
            // `check` says what it found; only a failure is left to say here.
            if let Err(err) = check(&app).await {
                tracing::info!("could not look for updates: {err}");
            }
            tokio::time::sleep(BETWEEN_CHECKS).await;
        }
    });
}

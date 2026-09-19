pub mod champions;
mod commands;
pub mod engine;
pub mod lcu;
mod logging;
pub mod platform;
pub mod profiles;
pub mod settings;
mod tray;

use tauri::{Manager, WindowEvent};

use champions::Champions;
use engine::Engine;
use profiles::Store;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = platform::data_dir().expect("APPDATA is set on Windows");

    // The app is still useful without a log file, so a failure here is not fatal.
    if let Err(err) = logging::init(&data_dir.join("logs")) {
        eprintln!("could not start logging: {err}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(tray::Notice::default())
        .manage(tray::Flyouts::default())
        .invoke_handler(tauri::generate_handler![
            tray::current_notice,
            commands::view,
            commands::champions,
            commands::apply_profile,
            commands::set_auto_apply,
            commands::rename_profile,
            commands::delete_profile,
            commands::save_current,
            commands::undo_last,
            commands::drift,
            commands::resolve_drift,
            commands::copy_riot_id,
            commands::open_manager,
            commands::open_logs,
            commands::notify,
            commands::dismiss,
            commands::quit,
        ])
        .setup(move |app| {
            // The engine needs Tauri's Tokio runtime to be current when it spawns.
            let engine = tauri::async_runtime::block_on(async { Engine::spawn(Store::new(&data_dir), Champions::new(&data_dir)) });
            app.manage(engine.clone());
            tray::init(app.handle(), engine)?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // mimic lives in the tray: closing the manager hides it.
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            // The tray flyouts behave like menus: clicking anywhere else dismisses them.
            WindowEvent::Focused(false) => {
                if let Some(flyout) = tray::Flyout::from_label(window.label()) {
                    tray::hide_flyout_if_inactive(window.clone(), flyout);
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Keep running with no windows open; only the tray's Quit exits.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

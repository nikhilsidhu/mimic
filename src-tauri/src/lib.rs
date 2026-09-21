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
        // First, so that a second launch ends before it does anything: it hands over to
        // the running one, which shows the manager.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_manager(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(tray::Notice::default())
        .manage(tray::PanelHidden::default())
        .invoke_handler(tauri::generate_handler![
            tray::current_notice,
            commands::view,
            commands::champions,
            commands::apply_profile,
            commands::set_auto_apply,
            commands::rename_profile,
            commands::delete_profile,
            commands::update_profile,
            commands::duplicate_profile,
            commands::profile_details,
            commands::export_profile,
            commands::import_profile,
            commands::delete_overlay,
            commands::overlay_sources,
            commands::save_overlay,
            commands::snapshots,
            commands::restore_snapshot,
            commands::accounts,
            commands::set_account_auto_apply,
            commands::forget_account,
            commands::save_current,
            commands::undo_last,
            commands::drift,
            commands::resolve_drift,
            commands::copy_riot_id,
            commands::install_path,
            commands::choose_install,
            commands::autostart,
            commands::set_autostart,
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
            // The tray panel behaves like a menu: clicking anywhere else dismisses it.
            WindowEvent::Focused(false) if window.label() == tray::PANEL => {
                tray::hide_panel_if_inactive(window.clone());
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

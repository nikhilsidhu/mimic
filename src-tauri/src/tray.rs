//! The tray icon and the small windows that open from it: the panel, and the notice
//! and changed-settings popups.
//!
//! Windows draws native tray menus itself and they cannot be styled, so the panel is a
//! frameless web window that behaves like a menu: it opens at the icon, on either mouse
//! button, and closes as soon as it loses focus.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::engine::{Announcement, Engine};

/// Gap between a window and the edges of the work area, in logical pixels.
const MARGIN: f64 = 12.0;
/// A tray click this soon after the panel hid itself is the click that hid it.
const REOPEN_GUARD: Duration = Duration::from_millis(250);
/// How long after a "lost focus" event the panel is checked for still being inactive.
const BLUR_SETTLE: Duration = Duration::from_millis(100);

/// The panel's window label, which is also its frontend route.
pub const PANEL: &str = "panel";
const PANEL_SIZE: (f64, f64) = (320.0, 400.0);
const DRIFT_SIZE: (f64, f64) = (380.0, 376.0);
/// The least and the most the prompt's height is fitted to; past the most, its list scrolls.
const DRIFT_HEIGHT: (f64, f64) = (220.0, 560.0);

/// When the panel was last hidden. Clicking the tray icon while the panel is open first
/// takes its focus away, which hides it; without this the same click would then open
/// it again.
#[derive(Default)]
pub struct PanelHidden(Mutex<Option<Instant>>);

/// The message the notice popup shows; the page fetches it when it loads.
#[derive(Default)]
pub struct Notice(Mutex<String>);

#[tauri::command]
pub fn current_notice(notice: tauri::State<Notice>) -> String {
    notice.0.lock().unwrap().clone()
}

pub fn init(app: &AppHandle, engine: Engine) -> tauri::Result<()> {
    let tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("the bundle has an icon").clone())
        .tooltip("mimic")
        .on_tray_icon_event(|tray, event| {
            // Either button: one panel does everything, so there is no second menu to
            // keep in step with it.
            let TrayIconEvent::Click {
                button: MouseButton::Left | MouseButton::Right,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            else {
                return;
            };
            if let Err(err) = toggle_panel(tray.app_handle(), position) {
                tracing::error!("could not show the panel: {err}");
            }
        })
        .build(app)?;

    // Created hidden up front so the first click opens it without a loading flash.
    panel_window(app)?;

    // What the engine does unasked, such as applying a profile at login, is announced.
    if let Some(mut notices) = engine.take_notices() {
        let app = app.clone();
        let engine = engine.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(announcement) = notices.recv().await {
                match announcement {
                    // Answers to what the user did go through `notify` and always show.
                    Announcement::Notice(message) if !engine.shows_notices() => {
                        tracing::info!("notice (not shown): {message}")
                    }
                    Announcement::Notice(message) | Announcement::Problem(message) => show_notice(&app, message),
                    Announcement::Drift => show_drift_prompt(&app),
                }
            }
        });
    }

    if crate::demo::enabled() {
        // Tall enough to show every section at once.
        if let Some(manager) = app.get_webview_window("main") {
            let _ = manager.set_size(tauri::LogicalSize::new(920.0, 1250.0));
        }
        show_manager(app);
        show_drift_prompt(app);
        toggle_panel(app, PhysicalPosition::new(0.0, 0.0))?;
    }

    // Keeps the tooltip current and tells open windows to refresh.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut status = engine.status.clone();
        let mut changes = engine.changes();
        loop {
            let label = status.borrow_and_update().label();
            changes.borrow_and_update();
            let _ = tray.set_tooltip(Some(format!("mimic\n{label}")));
            let _ = app.emit("view-changed", ());

            tokio::select! {
                changed = status.changed() => if changed.is_err() { break },
                changed = changes.changed() => if changed.is_err() { break },
            }
        }
    });
    Ok(())
}

fn panel_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(PANEL) {
        return Ok(window);
    }
    WebviewWindowBuilder::new(app, PANEL, WebviewUrl::App(PANEL.into()))
        .title("mimic")
        .inner_size(PANEL_SIZE.0, PANEL_SIZE.1)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
}

/// Opens the panel above the tray icon, or closes it if it is open. `click` is where
/// the icon was clicked.
fn toggle_panel(app: &AppHandle, click: PhysicalPosition<f64>) -> tauri::Result<()> {
    let window = panel_window(app)?;
    let just_hidden = app.state::<PanelHidden>().0.lock().unwrap().is_some_and(|at| at.elapsed() < REOPEN_GUARD);
    if window.is_visible()? || just_hidden {
        return hide_panel(app);
    }

    let monitor = window.monitor_from_point(click.x, click.y)?.or(window.primary_monitor()?);
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let (width, height) = ((PANEL_SIZE.0 * scale) as i32, (PANEL_SIZE.1 * scale) as i32);
        let margin = (MARGIN * scale) as i32;
        let area = monitor.work_area();
        let left = area.position.x + margin;
        let right = area.position.x + area.size.width as i32 - width - margin;
        // Centred on the icon, kept inside the work area, sitting on the taskbar.
        let x = (click.x as i32 - width / 2).clamp(left, right.max(left));
        let y = area.position.y + area.size.height as i32 - height - margin;
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    window.show()?;
    window.set_focus()
}

/// Reacts to the panel's "lost focus" event. On Windows that event also fires when the
/// first click inside moves keyboard focus from the window to the web view in it, and
/// hiding on that would swallow the click. So the window is looked at again a moment
/// later and only hidden if it really is no longer the active one.
pub fn hide_panel_if_inactive(window: tauri::Window) {
    // The demo keeps every window open so that they can be captured together.
    if crate::demo::enabled() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(BLUR_SETTLE).await;
        if !window.is_focused().unwrap_or(false) {
            let _ = hide_panel(window.app_handle());
        }
    });
}

/// Hides the panel; called when it loses focus, on Esc, or from its own buttons.
pub fn hide_panel(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(PANEL) {
        if window.is_visible()? {
            *app.state::<PanelHidden>().0.lock().unwrap() = Some(Instant::now());
            window.hide()?;
        }
    }
    Ok(())
}
pub fn show_manager(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Shows a short message next to the tray. The popup closes itself.
pub fn show_notice(app: &AppHandle, message: String) {
    tracing::info!("notice: {message}");
    // A page that is still loading fetches the message itself; one that is already
    // open gets it as an event.
    *app.state::<Notice>().0.lock().unwrap() = message.clone();
    if let Some(window) = app.get_webview_window("notice") {
        let _ = window.emit("notice", message);
    }
    if let Err(err) = show_popup(app, "notice", 360.0, 64.0) {
        tracing::error!("could not show a notice: {err}");
    }
}

/// Asks what to do about settings the user changed. The page reads the details itself
/// and is told to read them again if it was already open.
pub fn show_drift_prompt(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("drift") {
        let _ = window.emit("drift-changed", ());
    }
    if let Err(err) = show_popup(app, "drift", DRIFT_SIZE.0, DRIFT_SIZE.1) {
        tracing::error!("could not show the changed-settings prompt: {err}");
    }
}

/// Shows a frameless popup in the bottom-right corner of the primary monitor's work
/// area, above the taskbar. It never takes focus, so it cannot pull the user out of
/// whatever they are doing. `label` is also the frontend route.
/// Makes the prompt about changed settings as tall as what it shows, within reason, keeping it
/// in its corner. The page asks for this once it knows how much there is.
pub fn fit_drift_prompt(app: &AppHandle, height: f64) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("drift") else { return Ok(()) };
    window.set_size(tauri::LogicalSize::new(DRIFT_SIZE.0, height.clamp(DRIFT_HEIGHT.0, DRIFT_HEIGHT.1)))?;
    place_in_corner(&window)
}

/// Bottom right of the primary monitor's work area, above the taskbar.
fn place_in_corner(window: &WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = window.primary_monitor()? {
        let area = monitor.work_area();
        let size = window.outer_size()?;
        let margin = (MARGIN * monitor.scale_factor()) as i32;
        window.set_position(PhysicalPosition::new(
            area.position.x + area.size.width as i32 - size.width as i32 - margin,
            area.position.y + area.size.height as i32 - size.height as i32 - margin,
        ))?;
    }
    Ok(())
}

fn show_popup(app: &AppHandle, label: &str, width: f64, height: f64) -> tauri::Result<()> {
    let window = match app.get_webview_window(label) {
        Some(window) => window,
        None => WebviewWindowBuilder::new(app, label, WebviewUrl::App(label.into()))
            .title("mimic")
            .inner_size(width, height)
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .visible(false)
            .build()?,
    };

    place_in_corner(&window)?;
    window.show()
}

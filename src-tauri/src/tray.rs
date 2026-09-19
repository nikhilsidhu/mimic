//! The tray icon and the small windows that open from it: the panel (left click), the
//! context menu (right click) and the notice popup.
//!
//! Windows draws native tray menus itself and they cannot be styled, so both the panel
//! and the menu are frameless web windows that behave like menus: they open at the
//! icon and close as soon as they lose focus.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::engine::Engine;

/// Gap between a window and the edges of the work area, in logical pixels.
const MARGIN: f64 = 12.0;
/// A tray click this soon after a flyout hid itself is the click that hid it.
const REOPEN_GUARD: Duration = Duration::from_millis(250);
/// How long after a "lost focus" event a flyout is checked for still being inactive.
const BLUR_SETTLE: Duration = Duration::from_millis(100);

const PANEL_SIZE: (f64, f64) = (320.0, 400.0);

// The menu is sized to its content. These mirror the CSS in `routes/menu`.
const MENU_WIDTH: f64 = 220.0;
const MENU_ROW: f64 = 28.0;
const MENU_LABEL: f64 = 24.0;
const MENU_SEPARATOR: f64 = 9.0;
/// Padding plus border, top and bottom together.
const MENU_CHROME: f64 = 2.0 * (6.0 + 1.0);
/// Profiles shown before the list scrolls.
const MENU_MAX_PROFILES: usize = 8;

/// The windows that open from the tray icon and close on losing focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flyout {
    Panel,
    Menu,
}

impl Flyout {
    const ALL: [Flyout; 2] = [Flyout::Panel, Flyout::Menu];

    /// The window label, which is also the frontend route.
    pub fn label(self) -> &'static str {
        match self {
            Flyout::Panel => "panel",
            Flyout::Menu => "menu",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|flyout| flyout.label() == label)
    }
}

/// When each flyout was last hidden. Clicking the tray icon while a flyout is open
/// first takes its focus away, which hides it; without this the same click would then
/// open it again.
#[derive(Default)]
pub struct Flyouts(Mutex<HashMap<&'static str, Instant>>);

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
        .on_tray_icon_event({
            let engine = engine.clone();
            move |tray, event| {
                let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. } = event else {
                    return;
                };
                let flyout = match button {
                    MouseButton::Left => Flyout::Panel,
                    MouseButton::Right => Flyout::Menu,
                    _ => return,
                };
                if let Err(err) = toggle_flyout(tray.app_handle(), &engine, flyout, position) {
                    tracing::error!("could not show the {}: {err}", flyout.label());
                }
            }
        })
        .build(app)?;

    // Created hidden up front so the first click opens them without a loading flash.
    for flyout in Flyout::ALL {
        flyout_window(app, flyout)?;
    }

    // What the engine does unasked, such as applying a profile at login, is announced.
    if let Some(mut notices) = engine.take_notices() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(message) = notices.recv().await {
                show_notice(&app, message);
            }
        });
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

fn flyout_window(app: &AppHandle, flyout: Flyout) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(flyout.label()) {
        return Ok(window);
    }
    let (width, height) = PANEL_SIZE;
    WebviewWindowBuilder::new(app, flyout.label(), WebviewUrl::App(flyout.label().into()))
        .title("mimic")
        .inner_size(width, height)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
}

/// The menu's height for the number of profiles it lists.
fn menu_height(profiles: usize) -> f64 {
    // Open manager, Open logs folder, Quit.
    let actions = 3.0;
    let profile_block = match profiles.min(MENU_MAX_PROFILES) {
        0 => 0.0,
        shown => MENU_LABEL + MENU_ROW * shown as f64 + MENU_SEPARATOR,
    };
    MENU_CHROME + profile_block + MENU_ROW * actions + MENU_SEPARATOR
}

/// Opens a flyout at the tray icon, or closes it if it is open. `click` is where the
/// icon was clicked.
fn toggle_flyout(app: &AppHandle, engine: &Engine, flyout: Flyout, click: PhysicalPosition<f64>) -> tauri::Result<()> {
    // Only one at a time, like menus.
    for other in Flyout::ALL.into_iter().filter(|other| *other != flyout) {
        hide_flyout(app, other)?;
    }
    let window = flyout_window(app, flyout)?;
    let just_hidden =
        app.state::<Flyouts>().0.lock().unwrap().get(flyout.label()).is_some_and(|at| at.elapsed() < REOPEN_GUARD);
    if window.is_visible()? || just_hidden {
        return hide_flyout(app, flyout);
    }

    let (width, height) = match flyout {
        Flyout::Panel => PANEL_SIZE,
        Flyout::Menu => (MENU_WIDTH, menu_height(engine.profiles().map_or(0, |profiles| profiles.len()))),
    };
    window.set_size(LogicalSize::new(width, height))?;

    let monitor = window.monitor_from_point(click.x, click.y)?.or(window.primary_monitor()?);
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let (width, height, margin) = ((width * scale) as i32, (height * scale) as i32, (MARGIN * scale) as i32);
        let area = monitor.work_area();
        let (left, top) = (area.position.x + margin, area.position.y + margin);
        let right = area.position.x + area.size.width as i32 - width - margin;
        let bottom = area.position.y + area.size.height as i32 - height - margin;

        let (x, y) = match flyout {
            // Centred on the icon, sitting on the taskbar.
            Flyout::Panel => (click.x as i32 - width / 2, bottom),
            // Up and to the left of the cursor, where a native tray menu opens.
            Flyout::Menu => (click.x as i32 - width, click.y as i32 - height),
        };
        window.set_position(PhysicalPosition::new(x.clamp(left, right.max(left)), y.clamp(top, bottom.max(top))))?;
    }
    tracing::debug!(flyout = flyout.label(), "showing");
    window.show()?;
    window.set_focus()
}

/// Reacts to a flyout's "lost focus" event. On Windows that event also fires when the
/// first click inside moves keyboard focus from the window to the web view in it, and
/// hiding on that would swallow the click. So the window is looked at again a moment
/// later and only hidden if it really is no longer the active one.
pub fn hide_flyout_if_inactive(window: tauri::Window, flyout: Flyout) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(BLUR_SETTLE).await;
        if !window.is_focused().unwrap_or(false) {
            let _ = hide_flyout(window.app_handle(), flyout);
        }
    });
}

/// Hides a flyout; called when it loses focus or after one of its actions ran.
pub fn hide_flyout(app: &AppHandle, flyout: Flyout) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(flyout.label()) {
        if window.is_visible()? {
            tracing::debug!(flyout = flyout.label(), "hiding");
            app.state::<Flyouts>().0.lock().unwrap().insert(flyout.label(), Instant::now());
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

/// Shows a frameless popup in the bottom-right corner of the primary monitor's work
/// area, above the taskbar. It never takes focus, so it cannot pull the user out of
/// whatever they are doing. `label` is also the frontend route.
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

    if let Some(monitor) = window.primary_monitor()? {
        let area = monitor.work_area();
        let size = window.outer_size()?;
        let margin = (MARGIN * monitor.scale_factor()) as i32;
        window.set_position(PhysicalPosition::new(
            area.position.x + area.size.width as i32 - size.width as i32 - margin,
            area.position.y + area.size.height as i32 - size.height as i32 - margin,
        ))?;
    }
    window.show()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_grows_with_profiles_up_to_a_cap() {
        let empty = menu_height(0);
        assert_eq!(menu_height(1), empty + MENU_LABEL + MENU_ROW + MENU_SEPARATOR);
        assert_eq!(menu_height(3) - menu_height(2), MENU_ROW);
        assert_eq!(menu_height(MENU_MAX_PROFILES + 5), menu_height(MENU_MAX_PROFILES));
    }

    #[test]
    fn flyout_labels_round_trip() {
        for flyout in Flyout::ALL {
            assert_eq!(Flyout::from_label(flyout.label()), Some(flyout));
        }
        assert_eq!(Flyout::from_label("main"), None);
    }
}

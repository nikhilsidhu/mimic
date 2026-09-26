//! What mimic's windows cost while nobody is looking at them. Each window is a WebView2 page with
//! a renderer process of its own. The small ones, the tray panel and the popups, are kept for
//! when they are next needed, as they have to open at once; while hidden, WebView2 is asked to
//! hold as little memory for them as it can.

use tauri::WebviewWindow;

/// Tells WebView2 whether a window's page is out of sight. A page out of sight may have its caches
/// dropped and its memory paged out; showing it again brings it back. Does nothing where the
/// WebView2 runtime is too old to know the setting.
pub fn set_hidden(window: &WebviewWindow, hidden: bool) {
    #[cfg(windows)]
    let _ = window.with_webview(move |webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        };
        use windows_core::Interface;

        let level =
            if hidden { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
        // SAFETY: calls on the WebView2 controller Tauri hands over, on the thread it hands it
        // over on, as WebView2 requires.
        unsafe {
            let Ok(core) = webview.controller().CoreWebView2() else { return };
            let Ok(core) = core.cast::<ICoreWebView2_19>() else { return };
            if let Err(err) = core.SetMemoryUsageTargetLevel(level) {
                tracing::debug!("could not set a window's memory level: {err}");
            }
        }
    });
    #[cfg(not(windows))]
    let _ = (window, hidden);
}

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

static OVERLAY_PINNED: AtomicBool = AtomicBool::new(true); // overlay starts pinned

/// Toggle always-on-top for the overlay. Returns new pinned state.
pub fn toggle_pin(app: &AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window("overlay") {
        let current = OVERLAY_PINNED.load(Ordering::Relaxed);
        let new_value = !current;
        window
            .set_always_on_top(new_value)
            .map_err(|e| e.to_string())?;
        OVERLAY_PINNED.store(new_value, Ordering::Relaxed);
        Ok(new_value)
    } else {
        Err("Overlay not found".to_string())
    }
}

/// Set click-through mode on the overlay
pub fn set_click_through(app: &AppHandle, ignore: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_ignore_cursor_events(ignore)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Disable click-through and focus the overlay (escape hatch)
pub fn disable_click_through_and_focus(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.set_ignore_cursor_events(false);
        let _ = window.set_focus();
        let _ = window.emit("overlay-click-through-off", ());
    }
}

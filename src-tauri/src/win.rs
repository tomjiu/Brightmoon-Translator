//! Small Windows OS helpers shared across modules.
//!
//! S1-6: consolidates the four independent `GetCursorPos` wrappers that
//! previously lived in `selection/present.rs`, `selection/auto_watch.rs`,
//! `overlay/follow_controller.rs`, and `commands/window.rs` — each with a
//! different fallback coordinate and slightly different FFI style.

/// Return the current mouse cursor position in screen coordinates as
/// raw integers, or `None` if the cursor cannot be read.
///
/// Use this when you need to distinguish failure from a real position
/// (e.g. UIA `ElementFromPoint` callers return `None` on failure).
pub fn cursor_pos_raw() -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut pt = POINT::default();
        // SAFETY: GetCursorPos writes into a stack-allocated POINT. It is a
        // standard Win32 API with no preconditions beyond a valid pointer.
        if unsafe { GetCursorPos(&raw mut pt).is_ok() } {
            return Some((pt.x, pt.y));
        }
    }
    None
}

/// Return the current mouse cursor position in screen coordinates.
///
/// Returns `(100.0, 100.0)` when the cursor cannot be read (`GetCursorPos`
/// failure) or on non-Windows platforms. This fallback matches the previous
/// `present.rs` / `window.rs` behavior and keeps overlays on-screen rather
/// than pinned to the top-left corner `(0, 0)`.
pub fn cursor_pos() -> (f64, f64) {
    let (x, y) = cursor_pos_raw().unwrap_or((100, 100));
    (f64::from(x), f64::from(y))
}

// ── C3+C4: no-activate window styles ────────────────────────────────────────
//
// `WS_EX_NOACTIVATE` keeps a window from being activated when the user clicks
// it or when it is shown — the target app retains keyboard focus. This is
// essential for the OCR region frame and overlay cards in follow mode, where
// the source app must keep focus while the user reads the floating result.
//
// `SW_SHOWNOACTIVATE` shows a window without stealing the foreground. Used
// together with `WS_EX_NOACTIVATE` this gives a "floating panel" that the user
// can interact with (buttons still fire WM_COMMAND) but never becomes the
// active window.

/// Set or clear the `WS_EX_NOACTIVATE` extended style on a window.
///
/// Returns `true` if the style was applied successfully. On non-Windows
/// platforms or for a null HWND, returns `false`.
#[cfg(windows)]
pub fn set_window_no_activate(hwnd: isize, no_activate: bool) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
    };

    if hwnd == 0 {
        return false;
    }
    let hwnd = HWND(hwnd as *mut _);
    // SAFETY: hwnd is a live window handle. GetWindowLongPtrW/SetWindowLongPtrW
    // are standard Win32 APIs that read/write the window's extended style bits.
    // The only precondition is a valid HWND, which we checked above.
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let bit = WS_EX_NOACTIVATE.0 as isize;
        let new_style = if no_activate {
            ex_style | bit
        } else {
            ex_style & !bit
        };
        if new_style == ex_style {
            return true; // already in desired state
        }
        // SetWindowLongPtrW returns the previous value on success, 0 on failure
        // (use GetLastError to distinguish; 0 previous is rare for ex_style).
        let prev = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        prev != 0 || ex_style == 0
    }
}

#[cfg(not(windows))]
pub fn set_window_no_activate(_hwnd: isize, _no_activate: bool) -> bool {
    false
}

/// Show a window without activating it (`SW_SHOWNOACTIVATE`).
///
/// Returns `true` if the window was previously visible (Win32 `ShowWindow`
/// semantics), `false` if it was hidden or on non-Windows platforms.
#[cfg(windows)]
pub fn show_window_no_activate(hwnd: isize) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};

    if hwnd == 0 {
        return false;
    }
    let hwnd = HWND(hwnd as *mut _);
    // SAFETY: hwnd is a live window handle; ShowWindow is a standard Win32 API.
    unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE).as_bool() }
}

#[cfg(not(windows))]
pub fn show_window_no_activate(_hwnd: isize) -> bool {
    false
}

/// Apply `WS_EX_NOACTIVATE` to a Tauri webview window and show it without
/// activating. Convenience wrapper for the OCR region frame / overlay cards.
///
/// Returns `true` if the no-activate style was applied. The window is shown
/// regardless (via the Tauri `show()` path if the Win32 call fails).
#[cfg(windows)]
pub fn show_webview_no_activate(window: &tauri::WebviewWindow) -> bool {
    let hwnd = if let Ok(h) = window.hwnd() { h.0 as isize } else {
        let _ = window.show();
        return false;
    };
    let styled = set_window_no_activate(hwnd, true);
    show_window_no_activate(hwnd);
    styled
}

#[cfg(not(windows))]
pub fn show_webview_no_activate(window: &tauri::WebviewWindow) -> bool {
    let _ = window.show();
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_activate_helpers_handle_null_hwnd() {
        // Null HWND should not crash and should return false.
        assert!(!set_window_no_activate(0, true));
        assert!(!show_window_no_activate(0));
    }
}

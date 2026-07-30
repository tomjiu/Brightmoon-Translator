//! Keep floating UI from stealing foreground (Easydict PopButton: WS_EX_NOACTIVATE).
//! Critical for Windows Terminal multi-window: activating our chip would reshuffle tabs.

#![cfg(windows)]

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongW, SetWindowLongW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

/// Apply WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW and re-assert TOPMOST without activating.
pub fn apply_no_activate(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let h = HWND(hwnd as *mut _);
        let ex = GetWindowLongW(h, GWL_EXSTYLE);
        let new_ex = ex | (WS_EX_NOACTIVATE.0 as i32) | (WS_EX_TOOLWINDOW.0 as i32);
        if new_ex != ex {
            SetWindowLongW(h, GWL_EXSTYLE, new_ex);
        }
        let _ = SetWindowPos(
            h,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        );
    }
}

use super::{SelectionProvider, SelectionResult};

/// Gets selected text by simulating Ctrl+C and reading the clipboard.
/// Saves and restores original clipboard content.
/// Also reads the foreground window title for context.
pub struct ClipboardSelectionProvider;

#[async_trait::async_trait]
impl SelectionProvider for ClipboardSelectionProvider {
    async fn get_selection(&self) -> Option<SelectionResult> {
        // Wrap in spawn_blocking because get_clipboard_selection uses thread::sleep and Win32 clipboard APIs
        let result = tokio::task::spawn_blocking(get_clipboard_selection)
            .await
            .ok()
            .flatten();
        let (text, window_title) = result?;
        if text.trim().is_empty() {
            tracing::debug!("[clipboard] Got text but empty after trim");
            return None;
        }
        tracing::info!(
            "[clipboard] Got selection: {} chars from '{}'",
            text.trim().len(),
            window_title
        );
        Some(SelectionResult {
            text: text.trim().to_string(),
            source_app: detect_app_from_title(&window_title),
            window_title,
            bounds: None, // clipboard method cannot determine selection bounds
            confidence: 0.7,
            provider: "clipboard",
        })
    }

    fn name(&self) -> &'static str {
        "clipboard"
    }

    fn priority(&self) -> u32 {
        100 // low priority - fallback
    }
}

/// Simulate Ctrl+C, read clipboard, restore original content.
/// Returns (selected_text, foreground_window_title) or None on failure.
fn get_clipboard_selection() -> Option<(String, String)> {
    #[cfg(target_os = "windows")]
    {
        use std::mem::size_of;
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
            KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT,
            VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
        };

        extern "system" {
            fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
            fn CloseClipboard() -> i32;
            fn EmptyClipboard() -> i32;
            fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void)
                -> *mut std::ffi::c_void;
            fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
            fn GetClipboardSequenceNumber() -> u32;
            fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
            fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
            fn GlobalSize(hMem: *mut std::ffi::c_void) -> usize;
            fn GetForegroundWindow() -> *mut std::ffi::c_void;
        }

        const CF_UNICODETEXT: u32 = 13;
        const GMEM_MOVEABLE: u32 = 0x0002;

        fn make_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        wScan: 0,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }
        }

        /// Release stuck modifiers so hotkey chords do not break Ctrl+C (STranslate SendCtrlCV).
        unsafe fn release_modifiers() {
            let mods = [
                VK_CONTROL,
                VK_LCONTROL,
                VK_RCONTROL,
                VK_SHIFT,
                VK_LSHIFT,
                VK_RSHIFT,
                VK_MENU,
                VK_LMENU,
                VK_RMENU,
                VK_LWIN,
                VK_RWIN,
            ];
            let inputs: Vec<INPUT> = mods
                .iter()
                .map(|vk| make_input(*vk, KEYEVENTF_KEYUP))
                .collect();
            let _ = SendInput(&inputs, size_of::<INPUT>() as i32);
        }

        unsafe fn read_unicode_text() -> Option<String> {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return None;
            }
            let h_data = GetClipboardData(CF_UNICODETEXT);
            let text = if !h_data.is_null() {
                let p_data = GlobalLock(h_data);
                if !p_data.is_null() {
                    let size = GlobalSize(h_data);
                    let out = if size > 2 {
                        let slice = std::slice::from_raw_parts(p_data as *const u16, size / 2);
                        let text = String::from_utf16_lossy(slice);
                        Some(text.trim_end_matches('\0').to_string())
                    } else {
                        None
                    };
                    GlobalUnlock(h_data);
                    out
                } else {
                    None
                }
            } else {
                None
            };
            CloseClipboard();
            text
        }

        struct SyntheticGuard;
        impl Drop for SyntheticGuard {
            fn drop(&mut self) {
                crate::clipboard_dedupe::end_synthetic_clipboard();
            }
        }

        // SAFETY: Win32 clipboard and input simulation APIs.
        unsafe {
            crate::clipboard_dedupe::begin_synthetic_clipboard();
            let _synthetic = SyntheticGuard;

            let hwnd = GetForegroundWindow();
            let window_title = get_window_title(hwnd);

            let seq_before = GetClipboardSequenceNumber();
            let mut saved_text: Option<Vec<u8>> = None;
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                let h_data = GetClipboardData(CF_UNICODETEXT);
                if !h_data.is_null() {
                    let p_data = GlobalLock(h_data);
                    if !p_data.is_null() {
                        let size = GlobalSize(h_data);
                        if size > 2 {
                            let slice = std::slice::from_raw_parts(p_data as *const u8, size);
                            saved_text = Some(slice.to_vec());
                        }
                        GlobalUnlock(h_data);
                    }
                }
                CloseClipboard();
            } else {
                tracing::warn!("[clipboard] Failed to open clipboard for saving");
            }

            // STranslate-style: do not empty first; wait for sequence change after Ctrl+C.
            release_modifiers();
            std::thread::sleep(std::time::Duration::from_millis(20));

            let inputs = [
                make_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
                make_input(VK_C, KEYBD_EVENT_FLAGS(0)),
                make_input(VK_C, KEYEVENTF_KEYUP),
                make_input(VK_CONTROL, KEYEVENTF_KEYUP),
            ];
            let sent = SendInput(&inputs, size_of::<INPUT>() as i32);
            if sent == 0 {
                tracing::warn!(
                    "[clipboard] SendInput returned 0 — Ctrl+C may not have been delivered"
                );
            }

            // Wait for GetClipboardSequenceNumber change (10ms × 50 = 500ms)
            let mut seq_changed = false;
            for _ in 0..50 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                if GetClipboardSequenceNumber() != seq_before {
                    seq_changed = true;
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    break;
                }
            }
            if !seq_changed {
                tracing::debug!(
                    "[clipboard] Sequence did not change after 500ms; reading current clipboard"
                );
            }

            let selected_text = read_unicode_text().filter(|t| !t.trim().is_empty());

            // Restore original clipboard
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                EmptyClipboard();
                if let Some(ref saved) = saved_text {
                    let h_mem = GlobalAlloc(GMEM_MOVEABLE, saved.len());
                    if !h_mem.is_null() {
                        let p_mem = GlobalLock(h_mem);
                        if !p_mem.is_null() {
                            std::ptr::copy_nonoverlapping(
                                saved.as_ptr(),
                                p_mem as *mut u8,
                                saved.len(),
                            );
                            GlobalUnlock(h_mem);
                            SetClipboardData(CF_UNICODETEXT, h_mem);
                        } else {
                            tracing::warn!(
                                "[clipboard] GlobalLock failed when restoring clipboard"
                            );
                        }
                    } else {
                        tracing::warn!("[clipboard] GlobalAlloc failed when restoring clipboard");
                    }
                }
                CloseClipboard();
            } else {
                tracing::warn!("[clipboard] Failed to open clipboard for restore");
            }

            if let Some(ref t) = selected_text {
                crate::clipboard_dedupe::mark_clipboard_text(t);
            }

            return selected_text.map(|t| (t, window_title));
        }
    }

    #[cfg(not(target_os = "windows"))]
    None
}

/// Get window title from HWND.
/// SAFETY: GetWindowTextW is a standard Win32 API.
#[cfg(target_os = "windows")]
unsafe fn get_window_title(hwnd: *mut std::ffi::c_void) -> String {
    extern "system" {
        fn GetWindowTextW(hWnd: *mut std::ffi::c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
    }

    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        String::new()
    }
}

/// Extract a rough app name from the window title
fn detect_app_from_title(title: &str) -> String {
    if let Some(pos) = title.rfind(" - ") {
        let app = &title[pos + 3..];
        if !app.is_empty() {
            return app.to_string();
        }
    }
    title.to_string()
}

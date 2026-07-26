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
            KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_C, VK_CONTROL,
        };

        extern "system" {
            fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
            fn CloseClipboard() -> i32;
            fn EmptyClipboard() -> i32;
            fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void)
                -> *mut std::ffi::c_void;
            fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
            fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
            fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
            fn GlobalSize(hMem: *mut std::ffi::c_void) -> usize;
            fn GetForegroundWindow() -> *mut std::ffi::c_void;
        }

        const CF_UNICODETEXT: u32 = 13;
        const GMEM_MOVEABLE: u32 = 0x0002;

        // Use windows crate INPUT (40 bytes on x64) — manual type+ [u8;24] was 28 and broke SendInput.
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

        // SAFETY: Win32 clipboard and input simulation APIs.
        // Clipboard is saved/restored properly. SendInput simulates Ctrl+C.
        unsafe {
            // Get foreground window title
            let hwnd = GetForegroundWindow();
            let window_title = get_window_title(hwnd);

            // Save current clipboard content
            let mut clipboard_was_opened = false;
            let mut saved_text: Option<Vec<u8>> = None;

            if OpenClipboard(std::ptr::null_mut()) != 0 {
                clipboard_was_opened = true;
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

            // Clear clipboard before simulating Ctrl+C
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                EmptyClipboard();
                CloseClipboard();
            } else {
                tracing::warn!("[clipboard] Failed to open clipboard for clearing");
            }

            // Simulate Ctrl+C
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

            // Adaptive wait: poll clipboard every 50ms, up to 500ms
            let mut clipboard_ready = false;
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    let h_data = GetClipboardData(CF_UNICODETEXT);
                    let has_content = if !h_data.is_null() {
                        let p_data = GlobalLock(h_data);
                        let size = if !p_data.is_null() {
                            GlobalSize(h_data)
                        } else {
                            0
                        };
                        if !p_data.is_null() {
                            GlobalUnlock(h_data);
                        }
                        size > 2
                    } else {
                        false
                    };
                    CloseClipboard();
                    if has_content {
                        clipboard_ready = true;
                        break;
                    }
                }
            }
            if !clipboard_ready {
                tracing::debug!("[clipboard] Adaptive wait: clipboard did not get new content after 500ms, trying final read");
                // One last attempt with a bit more wait
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            // Read clipboard
            let selected_text = if OpenClipboard(std::ptr::null_mut()) != 0 {
                let h_data = GetClipboardData(CF_UNICODETEXT);
                let text = if !h_data.is_null() {
                    let p_data = GlobalLock(h_data);
                    if !p_data.is_null() {
                        let size = GlobalSize(h_data);
                        if size > 2 {
                            let slice = std::slice::from_raw_parts(p_data as *const u16, size / 2);
                            let text = String::from_utf16_lossy(slice);
                            let text = text.trim_end_matches('\0').to_string();
                            GlobalUnlock(h_data);
                            Some(text)
                        } else {
                            GlobalUnlock(h_data);
                            None
                        }
                    } else {
                        tracing::warn!("[clipboard] GlobalLock failed when reading clipboard");
                        None
                    }
                } else {
                    None
                };
                CloseClipboard();
                text
            } else {
                tracing::warn!("[clipboard] Failed to open clipboard for reading");
                None
            };

            // Restore clipboard
            if clipboard_was_opened {
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
                            tracing::warn!(
                                "[clipboard] GlobalAlloc failed when restoring clipboard"
                            );
                        }
                    }
                    CloseClipboard();
                } else {
                    tracing::warn!("[clipboard] Failed to open clipboard for restore");
                }
            }

            return selected_text.map(|t| (t, window_title));
        }
    }

    #[cfg(not(target_os = "windows"))]
    None
}

/// Get window title from HWND
#[cfg(target_os = "windows")]
/// Get window title from HWND.
/// SAFETY: GetWindowTextW is a standard Win32 API.
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
    // Common patterns: "Document - App Name", "App Name - something"
    if let Some(pos) = title.rfind(" - ") {
        let app = &title[pos + 3..];
        if !app.is_empty() {
            return app.to_string();
        }
    }
    title.to_string()
}

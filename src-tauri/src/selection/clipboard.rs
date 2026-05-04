use super::{SelectionBounds, SelectionProvider, SelectionResult};

/// Gets selected text by simulating Ctrl+C and reading the clipboard.
/// Saves and restores original clipboard content.
/// Also reads the foreground window title for context.
pub struct ClipboardSelectionProvider;

#[async_trait::async_trait]
impl SelectionProvider for ClipboardSelectionProvider {
    async fn get_selection(&self) -> Option<SelectionResult> {
        let (text, window_title) = get_clipboard_selection()?;
        if text.trim().is_empty() {
            return None;
        }
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
        use std::sync::atomic::{AtomicBool, Ordering};

        #[repr(C)]
        struct INPUT {
            type_: u32,
            union_data: [u8; 24],
        }

        #[repr(C)]
        struct KEYBDINPUT {
            wVk: u16,
            wScan: u16,
            dwFlags: u32,
            time: u32,
            dwExtraInfo: usize,
        }

        const INPUT_KEYBOARD: u32 = 1;
        const KEYEVENTF_KEYUP: u32 = 0x0002;
        const VK_CONTROL: u16 = 0x11;
        const VK_C: u16 = 0x43;

        extern "system" {
            fn SendInput(cInputs: u32, pInputs: *const INPUT, cbSize: i32) -> u32;
            fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
            fn CloseClipboard() -> i32;
            fn EmptyClipboard() -> i32;
            fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
            fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
            fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
            fn GlobalSize(hMem: *mut std::ffi::c_void) -> usize;
            fn GetForegroundWindow() -> *mut std::ffi::c_void;
            fn GetWindowTextW(hWnd: *mut std::ffi::c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
        }

        const CF_UNICODETEXT: u32 = 13;
        const GMEM_MOVEABLE: u32 = 0x0002;

        fn make_input(vk: u16, flags: u32) -> INPUT {
            let mut input = INPUT {
                type_: INPUT_KEYBOARD,
                union_data: [0u8; 24],
            };
            let ki = KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            };
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &ki as *const _ as *const u8,
                    input.union_data.as_mut_ptr(),
                    std::mem::size_of::<KEYBDINPUT>(),
                );
            }
            input
        }

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
            }

            // Clear clipboard before simulating Ctrl+C
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                EmptyClipboard();
                CloseClipboard();
            }

            // Simulate Ctrl+C
            let inputs = [
                make_input(VK_CONTROL, 0),
                make_input(VK_C, 0),
                make_input(VK_C, KEYEVENTF_KEYUP),
                make_input(VK_CONTROL, KEYEVENTF_KEYUP),
            ];
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );

            // Wait for clipboard
            std::thread::sleep(std::time::Duration::from_millis(150));

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
                        None
                    }
                } else {
                    None
                };
                CloseClipboard();
                text
            } else {
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
                            }
                        }
                    }
                    CloseClipboard();
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

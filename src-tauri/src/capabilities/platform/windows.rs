/// Set clipboard text content.
/// SAFETY: Win32 clipboard API calls. Clipboard is properly opened/closed.
unsafe fn set_clipboard_text(text: &str) -> Result<(), String> {
    extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
        fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
    }

    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;

    if OpenClipboard(std::ptr::null_mut()) == 0 {
        return Err("Failed to open clipboard for writing".to_string());
    }

    EmptyClipboard();

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let size = wide.len() * 2;

    let h_mem = GlobalAlloc(GMEM_MOVEABLE, size);
    if h_mem.is_null() {
        CloseClipboard();
        return Err(format!("GlobalAlloc failed for {size} bytes"));
    }

    let p_mem = GlobalLock(h_mem);
    if p_mem.is_null() {
        GlobalUnlock(h_mem);
        CloseClipboard();
        return Err("GlobalLock failed when setting clipboard".to_string());
    }

    std::ptr::copy_nonoverlapping(wide.as_ptr(), p_mem.cast::<u16>(), wide.len());
    GlobalUnlock(h_mem);

    let h_result = SetClipboardData(CF_UNICODETEXT, h_mem);
    if h_result.is_null() {
        // SetClipboardData returns NULL on failure; the handle is freed by the system on failure
        CloseClipboard();
        return Err("SetClipboardData failed".to_string());
    }

    CloseClipboard();
    Ok(())
}

fn make_key_input(
    vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS,
) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT};
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

/// Release stuck modifiers so hotkey chords do not break Ctrl+V / typing (`STranslate`).
fn release_modifiers() {
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, KEYEVENTF_KEYUP, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
        VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };
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
    let inputs: Vec<_> = mods
        .iter()
        .map(|vk| make_key_input(*vk, KEYEVENTF_KEYUP))
        .collect();
    // SAFETY: SendInput takes a slice of INPUT structs of known length and
    // the correct struct size. No preconditions beyond a valid pointer.
    unsafe {
        let _ = SendInput(
            &inputs,
            size_of::<windows::Win32::UI::Input::KeyboardAndMouse::INPUT>() as i32,
        );
    }
}

struct SyntheticClipboardGuard;
impl Drop for SyntheticClipboardGuard {
    fn drop(&mut self) {
        crate::clipboard_dedupe::end_synthetic_clipboard();
    }
}

/// Replace text in the foreground application via clipboard + Ctrl+V simulation.
/// Saves and restores the original clipboard content.
pub fn replace_text_via_clipboard(text: &str) -> Result<(), String> {
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };

    // M4-03: serialize with other clipboard readers/writers (hook monitor,
    // selection Ctrl+C) for the whole save→set→paste→restore window.
    let _clip_lock = crate::clipboard_dedupe::clipboard_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    const CF_UNICODETEXT: u32 = 13;

    extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
        fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
        fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
        fn GlobalSize(hMem: *mut std::ffi::c_void) -> usize;
    }

    crate::clipboard_dedupe::begin_synthetic_clipboard();
    let _synthetic = SyntheticClipboardGuard;

    // SAFETY: Win32 clipboard and input simulation APIs.
    unsafe {
        let saved_text = if OpenClipboard(std::ptr::null_mut()) != 0 {
            let h_data = GetClipboardData(CF_UNICODETEXT);
            let saved = if h_data.is_null() {
                None
            } else {
                let p_data = GlobalLock(h_data);
                if p_data.is_null() {
                    tracing::warn!("[replace] save clipboard: GlobalLock failed");
                    None
                } else {
                    let size = GlobalSize(h_data);
                    let slice = std::slice::from_raw_parts(p_data as *const u8, size);
                    let saved = slice.to_vec();
                    GlobalUnlock(h_data);
                    Some(saved)
                }
            };
            CloseClipboard();
            saved
        } else {
            tracing::warn!("[replace] save clipboard: OpenClipboard failed");
            None
        };

        set_clipboard_text(text).map_err(|e| {
            tracing::error!("[replace] set translated clipboard failed: {}", e);
            format!("set translated clipboard failed: {e}")
        })?;

        release_modifiers();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let inputs = [
            make_key_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
            make_key_input(VK_V, KEYBD_EVENT_FLAGS(0)),
            make_key_input(VK_V, KEYEVENTF_KEYUP),
            make_key_input(VK_CONTROL, KEYEVENTF_KEYUP),
        ];

        let sent = SendInput(
            &inputs,
            size_of::<windows::Win32::UI::Input::KeyboardAndMouse::INPUT>() as i32,
        );
        if sent == 0 {
            tracing::warn!("[replace] paste delivery uncertain: SendInput returned 0");
        }

        let paste_confirmed = {
            let mut confirmed = false;
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(30));
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    let h_data = GetClipboardData(CF_UNICODETEXT);
                    let has_content = if h_data.is_null() {
                        false
                    } else {
                        let p_data = GlobalLock(h_data);
                        let size = if p_data.is_null() {
                            0
                        } else {
                            GlobalSize(h_data)
                        };
                        if !p_data.is_null() {
                            GlobalUnlock(h_data);
                        }
                        size > 2
                    };
                    CloseClipboard();
                    if has_content {
                        confirmed = true;
                        break;
                    }
                }
            }
            confirmed
        };
        if !paste_confirmed {
            tracing::debug!("[replace] paste delivery uncertain: not confirmed after 300ms");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        if OpenClipboard(std::ptr::null_mut()) != 0 {
            EmptyClipboard();

            if let Some(saved) = saved_text {
                const GMEM_MOVEABLE: u32 = 0x0002;
                let h_mem = GlobalAlloc(GMEM_MOVEABLE, saved.len());
                if h_mem.is_null() {
                    tracing::warn!("[replace] restore clipboard: GlobalAlloc failed");
                } else {
                    let p_mem = GlobalLock(h_mem);
                    if p_mem.is_null() {
                        tracing::warn!("[replace] restore clipboard: GlobalLock failed");
                    } else {
                        std::ptr::copy_nonoverlapping(
                            saved.as_ptr(),
                            p_mem.cast::<u8>(),
                            saved.len(),
                        );
                        GlobalUnlock(h_mem);
                        SetClipboardData(CF_UNICODETEXT, h_mem);
                    }
                }
            }

            CloseClipboard();
        } else {
            tracing::warn!("[replace] restore clipboard: OpenClipboard failed");
        }
    }

    crate::clipboard_dedupe::mark_clipboard_text(text);
    tracing::info!(
        "[replace] Replace-via-clipboard completed for {} chars",
        text.len()
    );
    Ok(())
}

/// Type text into the foreground app via Unicode `SendInput` (no clipboard clobber).
/// `cancel` polled between characters when provided.
pub fn type_text_via_sendinput(
    text: &str,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    use std::mem::size_of;
    use std::sync::atomic::Ordering;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
        VIRTUAL_KEY,
    };

    release_modifiers();
    std::thread::sleep(std::time::Duration::from_millis(20));

    fn unicode_input(ch: u16, up: bool) -> INPUT {
        let mut flags = KEYEVENTF_UNICODE;
        if up {
            flags |= KEYEVENTF_KEYUP;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: ch,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    for ch in text.encode_utf16() {
        if cancel.is_some_and(|c| c.load(Ordering::Acquire)) {
            return Err("cancelled".to_string());
        }
        // Surrogate pairs need down/up for each code unit.
        let inputs = [unicode_input(ch, false), unicode_input(ch, true)];
        // SAFETY: SendInput takes a 2-element INPUT slice with the correct
        // struct size. Stack-allocated, no preconditions.
        let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if sent == 0 {
            return Err("SendInput type failed".to_string());
        }
        if ch == u16::from(b'\n') || ch == u16::from(b'\r') {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    tracing::info!(
        "[replace] Type-via-sendinput completed for {} chars",
        text.len()
    );
    Ok(())
}

/// Deliver replacement text: clipboard paste (default) or direct type.
pub fn deliver_replacement_text(
    text: &str,
    use_clipboard_output: bool,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    if use_clipboard_output {
        replace_text_via_clipboard(text)
    } else {
        type_text_via_sendinput(text, cancel)
    }
}

/// Information about the foreground application detected via Win32 APIs.
pub struct ForegroundAppInfo {
    pub app_name: String,
    pub window_title: String,
    pub pid: u32,
    pub window_class: String,
}

/// Detect the foreground application using Win32 APIs.
/// Returns process name, window title, PID, and window class name.
pub fn detect_foreground_app() -> Option<ForegroundAppInfo> {
    extern "system" {
        fn GetForegroundWindow() -> *mut std::ffi::c_void;
        fn GetWindowThreadProcessId(hWnd: *mut std::ffi::c_void, lpdwProcessId: *mut u32) -> u32;
        fn GetWindowTextW(hWnd: *mut std::ffi::c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
        fn GetClassNameW(hWnd: *mut std::ffi::c_void, lpClassName: *mut u16, nMaxCount: i32)
            -> i32;
        fn OpenProcess(
            dwDesiredAccess: u32,
            bInheritHandle: i32,
            dwProcessId: u32,
        ) -> *mut std::ffi::c_void;
        fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
        fn QueryFullProcessImageNameW(
            hProcess: *mut std::ffi::c_void,
            dwFlags: u32,
            lpExeName: *mut u16,
            lpdwSize: *mut u32,
        ) -> i32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    // SAFETY: Win32 API calls for foreground app detection.
    // All handles are properly closed.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        // Get PID
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &raw mut pid);
        if pid == 0 {
            return None;
        }

        // Get window title
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 512);
        let window_title = if title_len > 0 {
            String::from_utf16_lossy(&title_buf[..title_len as usize])
        } else {
            String::new()
        };

        // Get window class name
        let mut class_buf = [0u16; 256];
        let class_len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256);
        let window_class = if class_len > 0 {
            String::from_utf16_lossy(&class_buf[..class_len as usize])
        } else {
            String::new()
        };

        // Get process executable name
        let app_name = {
            let h_process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h_process.is_null() {
                String::new()
            } else {
                let mut exe_buf = [0u16; 1024];
                let mut exe_size = 1024u32;
                let result =
                    QueryFullProcessImageNameW(h_process, 0, exe_buf.as_mut_ptr(), &raw mut exe_size);
                CloseHandle(h_process);

                if result != 0 && exe_size > 0 {
                    let full_path = String::from_utf16_lossy(&exe_buf[..exe_size as usize]);
                    // Extract just the filename from the full path
                    full_path
                        .rsplit('\\')
                        .next()
                        .unwrap_or(&full_path)
                        .to_string()
                } else {
                    String::new()
                }
            }
        };

        Some(ForegroundAppInfo {
            app_name,
            window_title,
            pid,
            window_class,
        })
    }
}


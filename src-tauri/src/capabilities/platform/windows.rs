/// Set clipboard text content. Returns Err on any critical failure.
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
        return Err(format!("GlobalAlloc failed for {} bytes", size));
    }

    let p_mem = GlobalLock(h_mem);
    if p_mem.is_null() {
        GlobalUnlock(h_mem);
        CloseClipboard();
        return Err("GlobalLock failed when setting clipboard".to_string());
    }

    std::ptr::copy_nonoverlapping(wide.as_ptr(), p_mem as *mut u16, wide.len());
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

/// Replace text in the foreground application via clipboard + Ctrl+V simulation.
/// Saves and restores the original clipboard content.
pub fn replace_text_via_clipboard(text: &str) -> Result<(), String> {
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
    const VK_V: u16 = 0x56;
    const CF_UNICODETEXT: u32 = 13;

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
    }

    unsafe {
        // Save current clipboard content
        let saved_text = if OpenClipboard(std::ptr::null_mut()) != 0 {
            let h_data = GetClipboardData(CF_UNICODETEXT);
            let saved = if !h_data.is_null() {
                let p_data = GlobalLock(h_data);
                if !p_data.is_null() {
                    let size = GlobalSize(h_data);
                    let slice = std::slice::from_raw_parts(p_data as *const u8, size);
                    let saved = slice.to_vec();
                    GlobalUnlock(h_data);
                    Some(saved)
                } else {
                    log::warn!("[replace] save clipboard: GlobalLock failed");
                    None
                }
            } else {
                None
            };
            CloseClipboard();
            saved
        } else {
            log::warn!("[replace] save clipboard: OpenClipboard failed");
            None
        };

        // Set translated text to clipboard — this is the critical path
        set_clipboard_text(text).map_err(|e| {
            log::error!("[replace] set translated clipboard failed: {}", e);
            format!("set translated clipboard failed: {}", e)
        })?;

        // Simulate Ctrl+V
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

        let inputs = [
            make_input(VK_CONTROL, 0),
            make_input(VK_V, 0),
            make_input(VK_V, KEYEVENTF_KEYUP),
            make_input(VK_CONTROL, KEYEVENTF_KEYUP),
        ];

        let sent = SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
        if sent == 0 {
            log::warn!("[replace] paste delivery uncertain: SendInput returned 0");
        }

        // Adaptive wait: poll clipboard to confirm paste completed, 30ms intervals, max 300ms
        let paste_confirmed = {
            let mut confirmed = false;
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(30));
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    let h_data = GetClipboardData(CF_UNICODETEXT);
                    let has_content = if !h_data.is_null() {
                        let p_data = GlobalLock(h_data);
                        let size = if !p_data.is_null() { GlobalSize(h_data) } else { 0 };
                        if !p_data.is_null() { GlobalUnlock(h_data); }
                        size > 2
                    } else {
                        false
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
            log::debug!("[replace] paste delivery uncertain: not confirmed after 300ms");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // Restore original clipboard (or clear if it was empty) — warn on failure, not fatal
        if OpenClipboard(std::ptr::null_mut()) != 0 {
            EmptyClipboard();

            if let Some(saved) = saved_text {
                const GMEM_MOVEABLE: u32 = 0x0002;
                let h_mem = GlobalAlloc(GMEM_MOVEABLE, saved.len());
                if !h_mem.is_null() {
                    let p_mem = GlobalLock(h_mem);
                    if !p_mem.is_null() {
                        std::ptr::copy_nonoverlapping(saved.as_ptr(), p_mem as *mut u8, saved.len());
                        GlobalUnlock(h_mem);
                        SetClipboardData(CF_UNICODETEXT, h_mem);
                    } else {
                        log::warn!("[replace] restore clipboard: GlobalLock failed");
                    }
                } else {
                    log::warn!("[replace] restore clipboard: GlobalAlloc failed");
                }
            }

            CloseClipboard();
        } else {
            log::warn!("[replace] restore clipboard: OpenClipboard failed");
        }
    }

    log::info!("[replace] Replace-via-clipboard completed for {} chars", text.len());
    Ok(())
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
        fn GetWindowThreadProcessId(
            hWnd: *mut std::ffi::c_void,
            lpdwProcessId: *mut u32,
        ) -> u32;
        fn GetWindowTextW(hWnd: *mut std::ffi::c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
        fn GetClassNameW(hWnd: *mut std::ffi::c_void, lpClassName: *mut u16, nMaxCount: i32) -> i32;
        fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
        fn QueryFullProcessImageNameW(
            hProcess: *mut std::ffi::c_void,
            dwFlags: u32,
            lpExeName: *mut u16,
            lpdwSize: *mut u32,
        ) -> i32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        // Get PID
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
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
                let result = QueryFullProcessImageNameW(h_process, 0, exe_buf.as_mut_ptr(), &mut exe_size);
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

/// Classify the embedded app type based on process name and window class.
/// Returns None for standard (non-embedded) applications.
pub fn classify_embedded_app(app_name: &str, window_class: &str) -> Option<super::super::EmbeddedAppType> {
    let app_lower = app_name.to_lowercase();
    let class_lower = window_class.to_lowercase();

    // Electron apps: process name often contains "electron" or is a known Electron app
    // Window class is typically "Chrome_WidgetWin_1"
    if app_lower.contains("electron")
        || app_lower == "slack.exe"
        || app_lower == "discord.exe"
        || app_lower == "visual studio code.exe"
        || app_lower == "code.exe"
        || app_lower == "notion.exe"
        || app_lower == "figma.exe"
        || app_lower == "postman.exe"
        || app_lower == "spotify.exe"
        || app_lower == "microsoft teams.exe"
        || app_lower == "teams.exe"
    {
        return Some(super::super::EmbeddedAppType::Electron);
    }

    // WebView2: uses "Chrome_WidgetWin_1" class but is embedded in a host app
    // Detection is heuristic: apps with WebView2 runtime but not Electron
    if class_lower == "chrome_widgetwin_1" {
        // If it's a known WebView2 host, classify as WebView2
        if app_lower.contains("webview") || app_lower.contains("microsoftedge")
            || app_lower == "msedge.exe"
        {
            return Some(super::super::EmbeddedAppType::WebView2);
        }
        // If it has Chrome_WidgetWin_1 but isn't clearly Electron or WebView2,
        // it's likely a Chromium-based embedded app - default to Electron
        return Some(super::super::EmbeddedAppType::Electron);
    }

    // CEF (Chromium Embedded Framework): uses "Chrome_WidgetWin_0" class
    if class_lower == "chrome_widgetwin_0" {
        return Some(super::super::EmbeddedAppType::Cef);
    }

    None
}

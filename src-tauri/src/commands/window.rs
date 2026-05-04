use tauri::command;
use tauri::Manager;
use std::sync::atomic::{AtomicBool, Ordering};

static ALWAYS_ON_TOP: AtomicBool = AtomicBool::new(false);

#[command]
pub async fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[command]
pub async fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Get the currently selected text by simulating Ctrl+C and reading clipboard.
/// Saves and restores original clipboard content.
#[command]
pub async fn get_selected_text() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
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
            // Save current clipboard content
            // Track whether we opened clipboard (for restore), and what was in it
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

            // Simulate Ctrl+C to copy selected text
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

            // Wait for clipboard to be populated
            std::thread::sleep(std::time::Duration::from_millis(150));

            // Read clipboard (the selected text)
            let selected_text = if OpenClipboard(std::ptr::null_mut()) != 0 {
                let h_data = GetClipboardData(CF_UNICODETEXT);
                let text = if !h_data.is_null() {
                    let p_data = GlobalLock(h_data);
                    if !p_data.is_null() {
                        let size = GlobalSize(h_data);
                        if size > 2 {
                            let slice = std::slice::from_raw_parts(p_data as *const u16, size / 2);
                            let text = String::from_utf16_lossy(slice);
                            let text = text.trim_end_matches('\0');
                            GlobalUnlock(h_data);
                            Some(text.to_string())
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

            // Restore original clipboard (always, even if originally empty)
            if clipboard_was_opened {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    EmptyClipboard();
                    if let Some(ref saved) = saved_text {
                        let h_mem = GlobalAlloc(GMEM_MOVEABLE, saved.len());
                        if !h_mem.is_null() {
                            let p_mem = GlobalLock(h_mem);
                            if !p_mem.is_null() {
                                std::ptr::copy_nonoverlapping(saved.as_ptr(), p_mem as *mut u8, saved.len());
                                GlobalUnlock(h_mem);
                                SetClipboardData(CF_UNICODETEXT, h_mem);
                            }
                        }
                    }
                    // If saved_text was None, clipboard remains empty (already emptied above)
                    CloseClipboard();
                }
            }

            return selected_text.ok_or_else(|| "No text selected".to_string());
        }
    }

    #[cfg(not(target_os = "windows"))]
    Err("Not supported on this platform".to_string())
}

#[command]
pub async fn get_cursor_position() -> Result<(f64, f64), String> {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct POINT {
            x: i32,
            y: i32,
        }

        extern "system" {
            fn GetCursorPos(lpPoint: *mut POINT) -> i32;
        }

        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            if GetCursorPos(&mut point) != 0 {
                return Ok((point.x as f64, point.y as f64));
            }
        }
    }
    Ok((100.0, 100.0))
}

// Overlay HTML generation is now in crate::overlay::html_builder
// Overlay window management is now in crate::overlay::window_manager
// Overlay positioning is now in crate::overlay::positioner
// Overlay interaction (pin, click-through) is now in crate::overlay::interaction

#[command]
pub async fn create_overlay(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    text: String,
    show_controls: Option<bool>,
) -> Result<(), String> {
    let level = if show_controls.unwrap_or(false) {
        crate::overlay::OverlayLevel::Full
    } else {
        crate::overlay::OverlayLevel::Minimal
    };
    let content = crate::overlay::OverlayContent {
        source: String::new(),
        translated: text,
        source_app: None,
        window_title: None,
    };
    let html = crate::overlay::html_builder::build_html(&content, level, 3000);
    crate::overlay::window_manager::create_overlay_window(&app, &html, x, y, width, height, true)
}

#[command]
pub async fn close_overlay(app: tauri::AppHandle) -> Result<(), String> {
    crate::overlay::window_manager::close_overlay_window(&app);
    Ok(())
}

#[command]
pub async fn translate_selection(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    text: String,
    overlay_level: Option<u8>,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Text is empty".to_string());
    }

    let config = state.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
    let config_level = config.overlay_level;
    let dismiss_ms = config.overlay_auto_dismiss_ms;
    drop(config);

    let response = state.translation_service.translate(&text, &from, &to).await.map_err(|e| e.to_string())?;

    if let Some(first) = response.results.first() {
        let (cursor_x, cursor_y) = get_cursor_position().await.unwrap_or((100.0, 100.0));
        let pos = crate::overlay::OverlayPosition::at_cursor(cursor_x, cursor_y);

        let level: crate::overlay::OverlayLevel = overlay_level.unwrap_or(config_level).into();
        let content = crate::overlay::OverlayContent {
            source: text,
            translated: first.text.clone(),
            source_app: None,
            window_title: None,
        };
        let html = crate::overlay::html_builder::build_html(&content, level, dismiss_ms);
        crate::overlay::window_manager::create_overlay_window(
            &app, &html, pos.x, pos.y, pos.width, pos.height, true,
        )?;
    }

    Ok(())
}

/// Unified selection-translate entry point.
/// Uses SelectionProviderManager (UIA → clipboard fallback) to get text,
/// translates via TranslationService, and shows overlay.
#[command]
pub async fn trigger_selection_translate(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    overlay_level: Option<u8>,
) -> Result<(), String> {
    // Get selection via provider manager (UIA first, clipboard fallback)
    let selection = state.selection_manager.get_selection().await
        .ok_or_else(|| "No text selected".to_string())?;

    let config = state.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
    let config_level = config.overlay_level;
    let dismiss_ms = config.overlay_auto_dismiss_ms;
    drop(config);

    let response = state.translation_service.translate(&selection.text, &from, &to).await.map_err(|e| e.to_string())?;

    if let Some(first) = response.results.first() {
        // Position overlay: prefer selection bounds, fall back to cursor
        let (cursor_x, cursor_y) = get_cursor_position().await.unwrap_or((100.0, 100.0));
        let pos = crate::overlay::positioner::calculate_position(
            selection.bounds.as_ref(),
            cursor_x,
            cursor_y,
        );

        let level: crate::overlay::OverlayLevel = overlay_level.unwrap_or(config_level).into();
        let content = crate::overlay::OverlayContent {
            source: selection.text,
            translated: first.text.clone(),
            source_app: Some(selection.source_app),
            window_title: Some(selection.window_title),
        };
        let html = crate::overlay::html_builder::build_html(&content, level, dismiss_ms);
        crate::overlay::window_manager::create_overlay_window(
            &app, &html, pos.x, pos.y, pos.width, pos.height, true,
        )?;
    }

    Ok(())
}

#[command]
pub async fn set_overlay_click_through(app: tauri::AppHandle, ignore: bool) -> Result<(), String> {
    crate::overlay::interaction::set_click_through(&app, ignore)
}

#[command]
pub async fn pin_overlay(app: tauri::AppHandle) -> Result<bool, String> {
    crate::overlay::interaction::toggle_pin(&app)
}

#[command]
pub async fn move_overlay(app: tauri::AppHandle, x: f64, y: f64) -> Result<(), String> {
    crate::overlay::window_manager::move_overlay_window(&app, x, y);
    Ok(())
}

#[command]
pub async fn resize_overlay(app: tauri::AppHandle, width: f64, height: f64) -> Result<(), String> {
    crate::overlay::window_manager::resize_overlay_window(&app, width, height);
    Ok(())
}

#[command]
pub async fn toggle_always_on_top(app: tauri::AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window("main") {
        let current = ALWAYS_ON_TOP.load(Ordering::Relaxed);
        let new_value = !current;
        window
            .set_always_on_top(new_value)
            .map_err(|e| e.to_string())?;
        ALWAYS_ON_TOP.store(new_value, Ordering::Relaxed);
        Ok(new_value)
    } else {
        Err("Main window not found".to_string())
    }
}

#[command]
pub async fn get_always_on_top() -> Result<bool, String> {
    Ok(ALWAYS_ON_TOP.load(Ordering::Relaxed))
}

#[command]
pub async fn move_window_to_cursor(app: tauri::AppHandle) -> Result<(), String> {
    let (cursor_x, cursor_y) = get_cursor_position().await.unwrap_or((100.0, 100.0));

    if let Some(window) = app.get_webview_window("main") {
        // Position window near cursor with offset
        let window_x = cursor_x + 20.0;
        let window_y = cursor_y + 20.0;

        // Get screen size to keep window in bounds
        #[cfg(target_os = "windows")]
        {
            #[repr(C)]
            struct RECT {
                left: i32,
                top: i32,
                right: i32,
                bottom: i32,
            }
            extern "system" {
                fn GetWindowRect(hwnd: *mut std::ffi::c_void, rect: *mut RECT) -> i32;
            }
            extern "system" {
                fn GetSystemMetrics(nIndex: i32) -> i32;
            }
            const SM_CXSCREEN: i32 = 0;
            const SM_CYSCREEN: i32 = 1;

            unsafe {
                let screen_w = GetSystemMetrics(SM_CXSCREEN) as f64;
                let screen_h = GetSystemMetrics(SM_CYSCREEN) as f64;

                // Get window size
                let size = window.inner_size().unwrap_or(tauri::PhysicalSize::new(800, 600));
                let win_w = size.width as f64;
                let win_h = size.height as f64;

                // Keep in bounds
                let final_x = window_x.min(screen_w - win_w - 20.0).max(20.0);
                let final_y = window_y.min(screen_h - win_h - 20.0).max(20.0);

                let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(final_x as i32, final_y as i32)));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(window_x as i32, window_y as i32)));
        }

        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

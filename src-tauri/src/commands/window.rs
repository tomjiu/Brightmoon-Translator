use std::sync::atomic::{AtomicBool, Ordering};
use tauri::command;
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};

static ALWAYS_ON_TOP: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug)]
struct MonitorBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale_factor: f64,
}

fn monitor_scale_for_rect_center(
    monitors: &[MonitorBounds],
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    fallback_scale: f64,
) -> f64 {
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;

    monitors
        .iter()
        .find_map(|monitor| {
            let right = monitor.x + monitor.width;
            let bottom = monitor.y + monitor.height;

            if center_x >= monitor.x
                && center_x < right
                && center_y >= monitor.y
                && center_y < bottom
            {
                Some(monitor.scale_factor)
            } else {
                None
            }
        })
        .unwrap_or(fallback_scale)
}

#[cfg(test)]
mod tests {
    use super::{monitor_scale_for_rect_center, MonitorBounds};

    #[test]
    fn selects_scale_from_monitor_containing_rect_center() {
        let monitors = [
            MonitorBounds {
                x: -1280.0,
                y: 0.0,
                width: 1280.0,
                height: 1024.0,
                scale_factor: 1.25,
            },
            MonitorBounds {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
                scale_factor: 1.5,
            },
        ];

        let scale = monitor_scale_for_rect_center(&monitors, -900.0, 100.0, 300.0, 200.0, 1.0);

        assert_eq!(scale, 1.25);
    }

    #[test]
    fn falls_back_to_primary_scale_when_rect_center_is_outside_all_monitors() {
        let monitors = [MonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
            scale_factor: 1.5,
        }];

        let scale = monitor_scale_for_rect_center(&monitors, -500.0, -500.0, 100.0, 100.0, 2.0);

        assert_eq!(scale, 2.0);
    }
}

fn monitor_scale_for_physical_rect(
    app: &tauri::AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> f64 {
    let fallback_scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);

    app.available_monitors()
        .ok()
        .map(|monitors| {
            let bounds = monitors
                .into_iter()
                .map(|monitor| {
                    let pos = monitor.position();
                    let size = monitor.size();
                    MonitorBounds {
                        x: f64::from(pos.x),
                        y: f64::from(pos.y),
                        width: f64::from(size.width),
                        height: f64::from(size.height),
                        scale_factor: monitor.scale_factor(),
                    }
                })
                .collect::<Vec<_>>();

            monitor_scale_for_rect_center(&bounds, x, y, width, height, fallback_scale)
        })
        .unwrap_or(fallback_scale)
}

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
/// Wrapped in spawn_blocking because it uses thread::sleep and synchronous Win32 clipboard/input APIs.
#[command]
pub async fn get_selected_text() -> Result<String, String> {
    // Use spawn_blocking to avoid blocking the async runtime (150ms sleep + clipboard ops)
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            #[repr(C)]
            struct INPUT {
                type_: u32,
                union_data: [u8; 24],
            }

            #[repr(C)]
            #[allow(non_snake_case)]
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
                fn SetClipboardData(
                    uFormat: u32,
                    hMem: *mut std::ffi::c_void,
                ) -> *mut std::ffi::c_void;
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
                // SAFETY: copy_nonoverlapping for KEYBDINPUT into INPUT union.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        &ki as *const _ as *const u8,
                        input.union_data.as_mut_ptr(),
                        std::mem::size_of::<KEYBDINPUT>(),
                    );
                }
                input
            }

            // SAFETY: Win32 clipboard and input simulation APIs.
            // Clipboard is saved/restored properly. SendInput simulates Ctrl+C.
            // SAFETY: GetSystemMetrics is a standard Win32 API.
            unsafe {
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
                                let slice =
                                    std::slice::from_raw_parts(p_data as *const u16, size / 2);
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

                return selected_text.ok_or_else(|| "No text selected".to_string());
            }
        }

        #[cfg(not(target_os = "windows"))]
        Err("Not supported on this platform".to_string())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
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
        // SAFETY: GetCursorPos is a standard Win32 API. Buffer is stack-allocated.
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
pub async fn close_overlay(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    // Stop following before closing
    state.overlay.follow_controller.stop().await;
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

    let config = state.system.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
    let config_level = config.overlay_level;
    let dismiss_ms = config.overlay_auto_dismiss_ms;
    drop(config);

    let response = state
        .translation
        .service
        .translate(&text, &from, &to)
        .await
        .map_err(|e| e.to_string())?;

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
/// Delegates to the SelectionTranslation capability which composes
/// SelectionProviderManager -> TranslationService -> overlay.
#[command]
pub async fn trigger_selection_translate(
    state: tauri::State<'_, crate::AppState>,
    overlay_level: Option<u8>,
) -> Result<(), String> {
    let cap = state
        .selection_translation
        .get()
        .ok_or_else(|| "SelectionTranslation capability not initialized".to_string())?;

    let options = crate::capabilities::SelectionTranslateOptions {
        from: None,
        to: None,
        overlay_level,
        show_overlay: true,
    };

    cap.translate_selection(options)
        .await
        .map_err(|e| e.to_string())?;
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
            extern "system" {
                fn GetSystemMetrics(nIndex: i32) -> i32;
            }
            const SM_CXSCREEN: i32 = 0;
            const SM_CYSCREEN: i32 = 1;

            // SAFETY: GetSystemMetrics is a standard Win32 API.
            unsafe {
                let screen_w = GetSystemMetrics(SM_CXSCREEN) as f64;
                let screen_h = GetSystemMetrics(SM_CYSCREEN) as f64;

                // Get window size
                let size = window
                    .inner_size()
                    .unwrap_or(tauri::PhysicalSize::new(800, 600));
                let win_w = size.width as f64;
                let win_h = size.height as f64;

                // Keep in bounds
                let final_x = window_x.min(screen_w - win_w - 20.0).max(20.0);
                let final_y = window_y.min(screen_h - win_h - 20.0).max(20.0);

                let _ = window.set_position(tauri::Position::Physical(
                    tauri::PhysicalPosition::new(final_x as i32, final_y as i32),
                ));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                window_x as i32,
                window_y as i32,
            )));
        }

        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

/// Detect the foreground application and return its context.
/// Used by the frontend to understand what app the user is interacting with.
#[command]
pub async fn detect_foreground_app(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Option<crate::capabilities::AppContext>, String> {
    Ok(state.system.app_detector.detect().await)
}

/// Set the overlay follow mode.
/// Modes: "cursor", "target_bounds", "none"
#[command]
pub async fn set_overlay_follow_mode(
    state: tauri::State<'_, crate::AppState>,
    mode: String,
) -> Result<(), String> {
    use crate::overlay::FollowMode;
    let follow_mode = match mode.as_str() {
        "cursor" => FollowMode::Cursor,
        "target_bounds" | "target" => FollowMode::TargetBounds,
        _ => FollowMode::None,
    };
    state.overlay.follow_controller.set_mode(follow_mode).await;
    Ok(())
}

/// Refresh overlay position once (without starting continuous following).
#[command]
pub async fn refresh_overlay_position(
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    state.overlay.follow_controller.refresh_once().await;
    Ok(())
}

/// Stop overlay following (does not close the overlay).
#[command]
pub async fn stop_overlay_follow(state: tauri::State<'_, crate::AppState>) -> Result<(), String> {
    state.overlay.follow_controller.stop().await;
    Ok(())
}

/// Update overlay content in-place without rebuilding.
/// Preserves pin/click-through/follow state.
/// If overlay doesn't exist, creates it with the given position and text.
/// overlay_level: 1=Minimal, 2=Standard(copy+close), 3=Full(all controls). None=auto from show_controls.
#[command]
pub async fn update_overlay(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    text: String,
    show_controls: Option<bool>,
    source: Option<String>,
    overlay_level: Option<u8>,
) -> Result<(), String> {
    let exists = crate::overlay::window_manager::overlay_exists(&app);
    let source_text = source.unwrap_or_default();

    if exists {
        crate::overlay::window_manager::update_overlay_content(&app, &source_text, &text)?;
        crate::overlay::window_manager::update_overlay_position(&app, x, y)?;
        crate::overlay::window_manager::resize_overlay_window(&app, width, height);
    } else {
        let level = if let Some(lvl) = overlay_level {
            crate::overlay::OverlayLevel::from(lvl)
        } else if show_controls.unwrap_or(false) {
            crate::overlay::OverlayLevel::Full
        } else {
            crate::overlay::OverlayLevel::Standard
        };
        let content = crate::overlay::OverlayContent {
            source: source_text,
            translated: text,
            source_app: None,
            window_title: None,
        };
        let html = crate::overlay::html_builder::build_html(&content, level, 3000);
        crate::overlay::window_manager::create_overlay_window(
            &app, &html, x, y, width, height, true,
        )?;
    }

    Ok(())
}

/// Update only overlay content (text) without changing position.
/// Returns false if overlay doesn't exist.
#[command]
pub async fn update_overlay_content(
    app: tauri::AppHandle,
    source: String,
    translated: String,
) -> Result<bool, String> {
    crate::overlay::window_manager::update_overlay_content(&app, &source, &translated)
}

/// Update only overlay position without changing content.
/// Returns false if overlay doesn't exist.
#[command]
pub async fn update_overlay_position(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
) -> Result<bool, String> {
    crate::overlay::window_manager::update_overlay_position(&app, x, y)
}

/// Create (or re-create) the OCR region frame window at the specified screen position.
/// The region frame is a transparent, borderless, always-on-top window that shows a
/// draggable/resizable selection rectangle with OCR controls.
#[command]
pub async fn create_ocr_region_frame(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    // Close existing region frame if any
    if let Some(existing) = app.get_webview_window("ocr-region-frame") {
        let _ = existing.close();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let scale_factor = monitor_scale_for_physical_rect(&app, x, y, width, height);
    let toolbar_h_physical = 32.0 * scale_factor;
    let window_x = x.round() as i32;
    let window_y = (y - toolbar_h_physical).round() as i32;
    let window_w = width.max(80.0).round() as u32;
    let window_h = (height + toolbar_h_physical).max(60.0).round() as u32;
    let initial_logical_w = (window_w as f64 / scale_factor).max(80.0);
    let initial_logical_h = (window_h as f64 / scale_factor).max(60.0);

    tracing::info!(
        "Creating OCR region frame for capture ({}, {}) {}x{} (window physical: ({}, {}) {}x{}, scale: {})",
        x, y, width, height, window_x, window_y, window_w, window_h, scale_factor
    );

    // Retry loop: Tauri may not release the window label immediately after close()
    let max_attempts = 5;
    let mut last_error = String::new();

    for attempt in 1..=max_attempts {
        if attempt > 1 {
            let delay_ms = 50u64 * (1 << (attempt - 1)); // 50, 100, 200, 400, 800
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }

        match WebviewWindowBuilder::new(
            &app,
            "ocr-region-frame",
            WebviewUrl::App("index.html?window=ocr-region-frame".into()),
        )
        .title("OCR Region")
        .inner_size(initial_logical_w, initial_logical_h)
        .position(0.0, 0.0)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .focused(true)
        .fullscreen(false)
        .background_color(tauri::window::Color(10, 10, 10, 255))
        .build()
        {
            Ok(window) => {
                window
                    .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                        window_x, window_y,
                    )))
                    .map_err(|e| format!("Failed to position OCR region frame: {}", e))?;
                window
                    .set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                        window_w, window_h,
                    )))
                    .map_err(|e| format!("Failed to size OCR region frame: {}", e))?;
                tracing::info!(
                    "OCR region frame created successfully (attempt {})",
                    attempt
                );
                return Ok(());
            },
            Err(e) => {
                let err_str = e.to_string();
                last_error = err_str.clone();
                tracing::warn!(
                    "OCR region frame creation attempt {} failed: {}",
                    attempt,
                    err_str
                );
                // If it's not a label conflict, fail immediately
                if !err_str.contains("already exists") {
                    return Err(format!("Failed to create OCR region frame: {}", err_str));
                }
                // Otherwise retry with longer delay
            },
        }
    }

    Err(format!(
        "Failed to create OCR region frame after {} attempts: {}",
        max_attempts, last_error
    ))
}

/// Create the full-screen OCR screenshot selector window.
/// Uses explicit screen-sized dimensions for reliability instead of fullscreen mode,
/// which can be unreliable across Tauri v2 versions and multi-monitor setups.
#[command]
pub async fn create_ocr_screenshot_selector(app: tauri::AppHandle) -> Result<(), String> {
    // Close existing selector window if any, and wait briefly for cleanup
    if let Some(existing) = app.get_webview_window("ocr-screenshot") {
        let _ = existing.close();
        // Brief yield to let Tauri release the window label
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    tracing::info!("Creating OCR screenshot selector window");

    // Get virtual desktop dimensions in physical pixels so negative-origin
    // monitor layouts line up with the screenshot snapshot.
    #[cfg(target_os = "windows")]
    let (screen_x, screen_y, screen_w, screen_h) = {
        extern "system" {
            fn GetSystemMetrics(nIndex: i32) -> i32;
        }
        const SM_XVIRTUALSCREEN: i32 = 76;
        const SM_YVIRTUALSCREEN: i32 = 77;
        const SM_CXVIRTUALSCREEN: i32 = 78;
        const SM_CYVIRTUALSCREEN: i32 = 79;
        let physical_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) } as f64;
        let physical_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) } as f64;
        let physical_w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) } as f64;
        let physical_h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) } as f64;

        (physical_x, physical_y, physical_w, physical_h)
    };

    #[cfg(not(target_os = "windows"))]
    let (screen_x, screen_y, screen_w, screen_h) = (0.0, 0.0, 1920.0, 1080.0);

    let scale_factor =
        monitor_scale_for_physical_rect(&app, screen_x, screen_y, screen_w, screen_h);
    let initial_logical_w = (screen_w / scale_factor).max(100.0);
    let initial_logical_h = (screen_h / scale_factor).max(100.0);
    let physical_x = screen_x.round() as i32;
    let physical_y = screen_y.round() as i32;
    let physical_w = screen_w.max(100.0).round() as u32;
    let physical_h = screen_h.max(100.0).round() as u32;

    tracing::info!(
        "OCR selector bounds (physical): ({}, {}) {}x{} (scale: {})",
        physical_x,
        physical_y,
        physical_w,
        physical_h,
        scale_factor
    );

    let window = WebviewWindowBuilder::new(
        &app,
        "ocr-screenshot",
        WebviewUrl::App("index.html?window=ocr-screenshot".into()),
    )
    .title("OCR Screenshot")
    .inner_size(initial_logical_w, initial_logical_h)
    .position(0.0, 0.0)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(true)
    .visible(false) // Start hidden to avoid black flash
    .background_color(tauri::window::Color(0, 0, 0, 255))
    .build()
    .map_err(|e| format!("Failed to create OCR screenshot selector: {}", e))?;

    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            physical_x, physical_y,
        )))
        .map_err(|e| format!("Failed to position OCR screenshot selector: {}", e))?;
    window
        .set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            physical_w, physical_h,
        )))
        .map_err(|e| format!("Failed to size OCR screenshot selector: {}", e))?;

    tracing::info!("OCR screenshot selector window created successfully");
    Ok(())
}

/// Close the OCR screenshot selector window if it exists.
#[command]
pub async fn close_ocr_screenshot_selector(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("ocr-screenshot") {
        let _ = window.close();
        // Wait for the window to be fully destroyed before returning
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
}

/// Close the OCR region frame window if it exists.
#[command]
pub async fn close_ocr_region_frame(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("ocr-region-frame") {
        let _ = window.close();
        // Wait for the window to be fully destroyed before returning
        // This prevents ghost windows and "already exists" label conflicts
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
}

/// Show or hide the OCR region frame window.
/// Used to hide the frame before capturing screenshots to avoid capturing the frame itself.
#[command]
pub async fn set_ocr_region_frame_visible(
    app: tauri::AppHandle,
    visible: bool,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("ocr-region-frame") {
        if visible {
            let _ = window.show();
        } else {
            let _ = window.hide();
        }
    }
    Ok(())
}

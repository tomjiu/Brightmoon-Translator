use std::sync::atomic::{AtomicBool, Ordering};
use tauri::command;
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};

static ALWAYS_ON_TOP: AtomicBool = AtomicBool::new(false);

/// Force HWND so the *client* covers the virtual-screen rect.
/// Evidence: Tauri set outer to 1938x1090 when asked for 1920x1080 — DWM/chrome
/// padding. Use Tauri inner vs outer delta (no extra Win32 GetWindowRect — clashes).
///
/// `show`: when false, do **not** pass SWP_SHOWWINDOW. Selector is built
/// `visible(false)` and FE shows only after snapshot img loads; forcing SHOW
/// here painted near-black full-screen before the freeze image (OCR black screen).
#[cfg(target_os = "windows")]
fn force_hwnd_cover_physical(
    window: &tauri::WebviewWindow,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    show: bool,
) -> Result<(), String> {
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowPos(
            hwnd: isize,
            hwnd_insert_after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            uflags: u32,
        ) -> i32;
    }
    const HWND_TOPMOST: isize = -1;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;

    let hwnd = window
        .hwnd()
        .map_err(|e| format!("OCR selector hwnd: {e}"))?;
    let hwnd_raw = hwnd.0 as isize;

    // Seed placement via Tauri first so inner/outer metrics are valid.
    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x, y,
    )));
    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
        w as u32, h as u32,
    )));

    let outer_pos = window
        .outer_position()
        .map_err(|e| format!("outer_position: {e}"))?;
    let outer_size = window
        .outer_size()
        .map_err(|e| format!("outer_size: {e}"))?;
    let inner_pos = window
        .inner_position()
        .map_err(|e| format!("inner_position: {e}"))?;
    let inner_size = window
        .inner_size()
        .map_err(|e| format!("inner_size: {e}"))?;

    let pad_l = inner_pos.x - outer_pos.x;
    let pad_t = inner_pos.y - outer_pos.y;
    let pad_r = (outer_pos.x + outer_size.width as i32) - (inner_pos.x + inner_size.width as i32);
    let pad_b = (outer_pos.y + outer_size.height as i32) - (inner_pos.y + inner_size.height as i32);

    // Outer so that client origin is (x,y) and client size is (w,h).
    let outer_x = x - pad_l;
    let outer_y = y - pad_t;
    let outer_w = (w + pad_l + pad_r).max(1);
    let outer_h = (h + pad_t + pad_b).max(1);

    let mut flags = SWP_NOACTIVATE | SWP_FRAMECHANGED;
    if show {
        flags |= SWP_SHOWWINDOW;
    }

    // SAFETY: live Tauri HWND.
    let ok = unsafe {
        SetWindowPos(
            hwnd_raw,
            HWND_TOPMOST,
            outer_x,
            outer_y,
            outer_w,
            outer_h,
            flags,
        )
    };
    if ok == 0 {
        return Err("SetWindowPos returned FALSE".to_string());
    }

    let after_inner = window.inner_position().ok();
    let after_inner_sz = window.inner_size().ok();
    let after_outer = window.outer_position().ok();
    let after_outer_sz = window.outer_size().ok();
    tracing::info!(
        "OCR cover: show={}; want client=({},{}) {}x{}; pads LTRB=({},{},{},{}); outer_set=({},{}) {}x{}; after_inner={:?}/{:?}; after_outer={:?}/{:?}",
        show,
        x,
        y,
        w,
        h,
        pad_l,
        pad_t,
        pad_r,
        pad_b,
        outer_x,
        outer_y,
        outer_w,
        outer_h,
        after_inner,
        after_inner_sz,
        after_outer,
        after_outer_sz
    );
    Ok(())
}

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

/// Exclude/include a labeled window from BitBlt/DXGI capture without hide/show (anti black-flash).
/// Prefer this for main during OCR snip so the desktop is never blank while snapshot runs.
/// Returns true if affinity path succeeded.
#[command]
pub async fn set_window_exclude_from_capture(
    app: tauri::AppHandle,
    label: String,
    exclude: bool,
) -> Result<bool, String> {
    let Some(window) = app.get_webview_window(&label) else {
        return Ok(false);
    };

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
        };

        let hwnd = match window.hwnd() {
            Ok(h) => HWND(h.0 as *mut _),
            Err(_) => return Ok(false),
        };
        let affinity = if exclude {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };
        // SAFETY: live Tauri HWND.
        let ok = unsafe { SetWindowDisplayAffinity(hwnd, affinity) };
        if ok.is_err() {
            tracing::warn!(
                "SetWindowDisplayAffinity({label}, exclude={exclude}) failed: {:?}",
                ok
            );
            return Ok(false);
        }
        return Ok(true);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, exclude);
        Ok(false)
    }
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
            use std::mem::size_of;
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
                KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_C, VK_CONTROL,
            };

            extern "system" {
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

            // Use windows crate INPUT (40 bytes on x64) — manual type+[u8;24] was 28 and broke SendInput.
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
                    make_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
                    make_input(VK_C, KEYBD_EVENT_FLAGS(0)),
                    make_input(VK_C, KEYEVENTF_KEYUP),
                    make_input(VK_CONTROL, KEYEVENTF_KEYUP),
                ];
                SendInput(&inputs, size_of::<INPUT>() as i32);

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
        .run_full(
            crate::models::translation::TranslateChannel::Selection,
            &text,
            &from,
            &to,
        )
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

/// Pop button clicked → translate pending selection text and show overlay.
#[command]
pub async fn pop_button_confirm(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let text = crate::selection::pop_button::take_pending()
        .ok_or_else(|| "No pending selection".to_string())?;
    let _ = crate::selection::pop_button::dismiss(&app);
    translate_selection(app, state, text, None).await
}

/// Hide pop button without translating.
#[command]
pub async fn pop_button_dismiss(app: tauri::AppHandle) -> Result<(), String> {
    crate::selection::pop_button::dismiss(&app)
}

#[command]
pub async fn set_overlay_click_through(app: tauri::AppHandle, ignore: bool) -> Result<(), String> {
    crate::overlay::interaction::set_click_through(&app, ignore)
}

/// Sync main-window theme (dark|light) to selection/hover overlay cards.
#[command]
pub async fn set_overlay_theme(theme: String) -> Result<(), String> {
    let light = theme.eq_ignore_ascii_case("light");
    crate::overlay::window_manager::set_overlay_theme_light(light);
    Ok(())
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

/// Create (or re-use) the OCR region frame window at the specified screen position.
/// Reuses existing webview when possible (no destroy/100ms sleep) — faster re-snip.
#[command]
pub async fn create_ocr_region_frame(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let scale_factor = monitor_scale_for_physical_rect(&app, x, y, width, height);
    // Keep in sync with src/components/ocrRegionGeometry.ts (I2/I3).
    const OCR_TOOLBAR_CSS_PX: f64 = 32.0;
    // Keep in sync with ocrRegionGeometry.ts OCR_MIN_FRAME_WIDTH_CSS (I3).
    const OCR_MIN_FRAME_CSS_W: f64 = 460.0;
    let toolbar_h_physical = OCR_TOOLBAR_CSS_PX * scale_factor;
    let min_w_physical = (OCR_MIN_FRAME_CSS_W * scale_factor).max(200.0);
    let min_h_physical = toolbar_h_physical + (48.0 * scale_factor);
    // Min-width: expand symmetrically so chrome is not only larger on the right.
    // FE paints capture image centered in the content area (fitImageDisplayRect).
    let window_w = width.max(min_w_physical);
    let expand_x = (window_w - width).max(0.0);
    let window_x = (x - expand_x / 2.0).round() as i32;
    let window_y = (y - toolbar_h_physical).round() as i32;
    let window_w = window_w.round() as u32;
    let window_h = (height + toolbar_h_physical).max(min_h_physical).round() as u32;
    let initial_logical_w = (window_w as f64 / scale_factor).max(OCR_MIN_FRAME_CSS_W);
    let initial_logical_h = (window_h as f64 / scale_factor).max(80.0);

    tracing::info!(
        "Creating OCR region frame for capture ({}, {}) {}x{} (window physical: ({}, {}) {}x{}, scale: {}, expand_x: {})",
        x, y, width, height, window_x, window_y, window_w, window_h, scale_factor, expand_x
    );

    // Reuse existing frame: reposition only (no webview reload / label churn).
    // Pin CLIENT rect to capture (same DWM-pad fix as screenshot selector).
    if let Some(existing) = app.get_webview_window("ocr-region-frame") {
        #[cfg(target_os = "windows")]
        {
            if let Err(e) = force_hwnd_cover_physical(
                &existing,
                window_x,
                window_y,
                window_w as i32,
                window_h as i32,
                true,
            ) {
                tracing::warn!("OCR region frame force cover (reuse): {e}");
                let _ = existing.set_position(tauri::Position::Physical(
                    tauri::PhysicalPosition::new(window_x, window_y),
                ));
                let _ = existing.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                    window_w, window_h,
                )));
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            existing
                .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                    window_x, window_y,
                )))
                .map_err(|e| format!("Failed to position OCR region frame: {}", e))?;
            existing
                .set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                    window_w, window_h,
                )))
                .map_err(|e| format!("Failed to size OCR region frame: {}", e))?;
        }
        // Main stays collapsed for OCR session; frame becomes the foreground baton.
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.set_always_on_top(false);
            let _ = main.hide();
        }
        let _ = existing.set_always_on_top(true);
        let _ = existing.show();
        let _ = existing.set_focus();
        tracing::info!("OCR region frame reused (repositioned)");
        return Ok(());
    }

    // Retry loop: Tauri may not release the window label immediately after close()
    let max_attempts = 5;
    let mut last_error = String::new();

    for attempt in 1..=max_attempts {
        if attempt > 1 {
            let delay_ms = 50u64 * (1 << (attempt - 1));
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }

        match WebviewWindowBuilder::new(
            &app,
            "ocr-region-frame",
            WebviewUrl::App("index.html?window=ocr-region-frame&v=ocr2".into()),
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
        // Opaque dark first paint — transparent WebView2 flashes pure white on create.
        .transparent(false)
        .background_color(tauri::window::Color(12, 12, 14, 255))
        .visible(false)
        .build()
        {
            Ok(window) => {
                #[cfg(target_os = "windows")]
                {
                    if let Err(e) = force_hwnd_cover_physical(
                        &window,
                        window_x,
                        window_y,
                        window_w as i32,
                        window_h as i32,
                        false,
                    ) {
                        tracing::warn!("OCR region frame force cover: {e}");
                        let _ = window.set_position(tauri::Position::Physical(
                            tauri::PhysicalPosition::new(window_x, window_y),
                        ));
                        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                            window_w, window_h,
                        )));
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
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
                }
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.set_always_on_top(false);
                    let _ = main.hide();
                }
                let _ = window.set_always_on_top(true);
                // Keep hidden until FE shows after crop — avoids white WebView2 flash.
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
                if !err_str.contains("already exists") {
                    return Err(format!("Failed to create OCR region frame: {}", err_str));
                }
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
    // Always destroy previous selector so FE remounts and reloads the fresh snapshot.
    // "Reuse hidden webview" left a black full-screen window with no img reload.
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

    // Prefer primary-monitor scale for CSS/logical sizing — must match the webview
    // devicePixelRatio so the snapshot image (physical pixels) lines up with the desktop.
    // Mixing virtual-screen physical size with a wrong scale causes a right/down shift
    // in the selector overlay (user sees frozen desktop offset).
    let scale_factor =
        monitor_scale_for_physical_rect(&app, screen_x, screen_y, screen_w, screen_h);
    let physical_x = screen_x.round() as i32;
    let physical_y = screen_y.round() as i32;
    let physical_w = screen_w.max(100.0).round() as u32;
    let physical_h = screen_h.max(100.0).round() as u32;
    let initial_logical_w = (physical_w as f64 / scale_factor).max(100.0);
    let initial_logical_h = (physical_h as f64 / scale_factor).max(100.0);

    tracing::info!(
        "OCR selector bounds (physical): ({}, {}) {}x{} (scale: {}, logical: {}x{})",
        physical_x,
        physical_y,
        physical_w,
        physical_h,
        scale_factor,
        initial_logical_w,
        initial_logical_h
    );

    // Do NOT use set_fullscreen: on WebView2 it often paints pure black while the
    // page loads (main window is already hidden for OCR → user sees total black screen).
    // Cover the virtual desktop with explicit physical bounds instead.
    let window = WebviewWindowBuilder::new(
        &app,
        "ocr-screenshot",
        WebviewUrl::App("index.html?window=ocr-screenshot&v=ocr2".into()),
    )
    // Title is a build fingerprint — if the user does not see [OCR-v2] in the
    // page UI, they are not running this binary's frontend (wrong process / stale cache).
    .title("OCR-v2 Screenshot")
    .inner_size(initial_logical_w, initial_logical_h)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(true)
    .visible(false)
    // Dark gray — pure black was indistinguishable from freeze-load failure.
    .background_color(tauri::window::Color(17, 17, 17, 255))
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

    // Win32 pin: eliminate left-edge "real desktop strip" when Tauri leaves the
    // window slightly to the right of the virtual-screen origin.
    #[cfg(target_os = "windows")]
    {
        // show=false: keep visible(false) until FE img.onLoad → win.show().
        if let Err(e) = force_hwnd_cover_physical(
            &window,
            physical_x,
            physical_y,
            physical_w as i32,
            physical_h as i32,
            false,
        ) {
            tracing::warn!("OCR selector force cover (pre-show): {e}");
        }
    }

    if let (Ok(pos), Ok(size), Ok(scale)) = (
        window.outer_position(),
        window.outer_size(),
        window.scale_factor(),
    ) {
        tracing::info!(
            "OCR selector actual outer: pos=({}, {}) size={}x{} scale={}",
            pos.x,
            pos.y,
            size.width,
            size.height,
            scale
        );
    }

    // One delayed re-pin only (was 80ms+200ms double re-pin → visible flicker).
    // Still show=false so a late pin cannot flash black before the freeze image.
    #[cfg(target_os = "windows")]
    {
        let window2 = window.clone();
        let px = physical_x;
        let py = physical_y;
        let pw = physical_w as i32;
        let ph = physical_h as i32;
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if let Err(e) = force_hwnd_cover_physical(&window2, px, py, pw, ph, false) {
                tracing::warn!("OCR selector force cover (post): {e}");
            }
        });
    }

    tracing::info!("OCR screenshot selector window created successfully");
    Ok(())
}

/// Close the OCR screenshot selector.
///
/// Lifecycle (pot baton): if a region frame exists, it must already be shown —
/// we only re-assert topmost+focus, then destroy the selector so DWM has a
/// foreground successor other than `main`. Main stays hidden for the OCR session.
#[command]
pub async fn close_ocr_screenshot_selector(app: tauri::AppHandle) -> Result<(), String> {
    // Result takes the baton first (if present).
    if let Some(frame) = app.get_webview_window("ocr-region-frame") {
        let _ = frame.set_ignore_cursor_events(false);
        let _ = frame.set_always_on_top(true);
        let _ = frame.show();
        let _ = frame.set_focus();
    }
    if let Some(window) = app.get_webview_window("ocr-screenshot") {
        let _ = window.hide();
        let _ = window.close();
        tokio::time::sleep(std::time::Duration::from_millis(16)).await;
    }
    // Keep main out of the session (STranslate collapsed); do not show it here.
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_always_on_top(false);
        let _ = main.hide();
    }
    if let Some(frame) = app.get_webview_window("ocr-region-frame") {
        let _ = frame.set_always_on_top(true);
        let _ = frame.set_focus();
    }
    Ok(())
}

/// OCR session start (STranslate): collapse main for the whole session.
/// Main must not re-enter until `ocr_end_session_show_main`.
#[command]
pub async fn ocr_begin_session_hide_main(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_always_on_top(false);
        let _ = main.set_skip_taskbar(true);
        let _ = main.hide();
    }
    Ok(())
}

/// OCR session end: restore main only when the user closed the result or cancelled.
#[command]
pub async fn ocr_end_session_show_main(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_skip_taskbar(false);
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
    Ok(())
}

/// Close the OCR region frame window if it exists.
#[command]
pub async fn close_ocr_region_frame(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("ocr-region-frame") {
        let _ = window.close();
        // Short yield so label releases without long blank
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
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
            let _ = window.set_always_on_top(true);
            let _ = window.show();
            let _ = window.set_focus();
            // Do not hide main here — caller owns main visibility.
            // Hiding main from every "visible:true" left users on a black desktop when
            // the frame failed or was empty.
        } else {
            let _ = window.hide();
        }
    }
    Ok(())
}

/// Exclude/include OCR region frame from screen capture without hide/show flash (I1).
/// Uses Win32 `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` so BitBlt/DXGI skip this HWND
/// while the user still sees the frame. Falls back to hide/show if affinity fails.
///
/// Returns `true` if affinity path was used (short settle OK), `false` if hide/show fallback
/// (caller should wait longer for DWM, ~40–50ms).
///
/// When clearing sampling (`sampling=false`), does **not** force-show if the window is already
/// hidden (e.g. mid-session re-snip) — avoids popping the region frame over the selector.
#[command]
pub async fn set_ocr_region_frame_sampling(
    app: tauri::AppHandle,
    sampling: bool,
) -> Result<bool, String> {
    let Some(window) = app.get_webview_window("ocr-region-frame") else {
        return Ok(false);
    };

    let was_visible = window.is_visible().unwrap_or(true);

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
        };

        let hwnd = match window.hwnd() {
            Ok(h) => HWND(h.0 as *mut _),
            Err(_) => {
                if sampling {
                    let _ = window.hide();
                } else if was_visible {
                    let _ = window.show();
                }
                return Ok(false);
            },
        };

        let affinity = if sampling {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };
        // SAFETY: live Tauri window HWND.
        let ok = unsafe { SetWindowDisplayAffinity(hwnd, affinity) };
        if ok.is_err() {
            tracing::warn!(
                "SetWindowDisplayAffinity failed, falling back to hide/show: {:?}",
                ok
            );
            if sampling {
                let _ = window.hide();
            } else if was_visible {
                let _ = window.show();
            }
            return Ok(false);
        }
        // Affinity cleared: only show if we did not intentionally hide the frame.
        if !sampling && was_visible {
            let _ = window.show();
        }
        return Ok(true);
    }

    #[cfg(not(target_os = "windows"))]
    {
        if sampling {
            let _ = window.hide();
        } else if was_visible {
            let _ = window.show();
        }
        Ok(false)
    }
}

/// Click-through the OCR region frame so hwnd_from_point hits content underneath (I6 follow bind).
/// Does not hide the window — less flash than set_visible(false).
#[command]
pub async fn set_ocr_region_frame_click_through(
    app: tauri::AppHandle,
    ignore: bool,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("ocr-region-frame") {
        window
            .set_ignore_cursor_events(ignore)
            .map_err(|e| format!("set_ignore_cursor_events: {e}"))?;
    }
    Ok(())
}

/// Move/resize the OCR region frame so its *capture* rect matches the given
/// physical-pixel region (toolbar height is added above the capture area).
#[command]
pub async fn move_ocr_region_frame(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window("ocr-region-frame") else {
        return Err("OCR region frame is not open".to_string());
    };

    let scale_factor = monitor_scale_for_physical_rect(&app, x, y, width, height);
    // Same constants as create_ocr_region_frame / OcrRegionFrame.tsx (I2/I3)
    // Keep in sync with src/components/ocrRegionGeometry.ts
    // OCR_TOOLBAR_HEIGHT_CSS / OCR_MIN_FRAME_WIDTH_CSS (I2/I3).
    const OCR_TOOLBAR_CSS_PX: f64 = 32.0;
    const OCR_MIN_FRAME_CSS_W: f64 = 460.0;
    let toolbar_h_physical = OCR_TOOLBAR_CSS_PX * scale_factor;
    let min_w_physical = (OCR_MIN_FRAME_CSS_W * scale_factor).max(200.0);
    let min_h_physical = toolbar_h_physical + (48.0 * scale_factor);
    let window_w = width.max(min_w_physical);
    let expand_x = (window_w - width).max(0.0);
    let window_x = (x - expand_x / 2.0).round() as i32;
    let window_y = (y - toolbar_h_physical).round() as i32;
    let window_w = window_w.round() as u32;
    let window_h = (height + toolbar_h_physical).max(min_h_physical).round() as u32;

    #[cfg(target_os = "windows")]
    {
        if let Err(e) = force_hwnd_cover_physical(
            &window,
            window_x,
            window_y,
            window_w as i32,
            window_h as i32,
            true,
        ) {
            tracing::warn!("OCR region frame force cover (move): {e}");
            window
                .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                    window_x, window_y,
                )))
                .map_err(|e| format!("Failed to move OCR region frame: {}", e))?;
            window
                .set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                    window_w, window_h,
                )))
                .map_err(|e| format!("Failed to resize OCR region frame: {}", e))?;
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        window
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                window_x, window_y,
            )))
            .map_err(|e| format!("Failed to move OCR region frame: {}", e))?;
        window
            .set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                window_w, window_h,
            )))
            .map_err(|e| format!("Failed to resize OCR region frame: {}", e))?;
    }

    Ok(())
}

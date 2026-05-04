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

/// Overlay display level
/// L1: Minimal - translated text only, auto-dismiss
/// L2: Standard - source + translated, copy button
/// L3: Full - source + translated, all controls (copy, pin, click-through, close)
fn create_overlay_html(source: &str, translated: &str, level: u8, dismiss_ms: u64) -> String {
    match level {
        1 => create_l1_overlay(translated, dismiss_ms),
        2 => create_l2_overlay(source, translated),
        _ => create_l3_overlay(source, translated),
    }
}

/// L1: Minimal overlay - just translated text, auto-dismiss after dismiss_ms
fn create_l1_overlay(translated: &str, dismiss_ms: u64) -> String {
    let escaped = html_escape::encode_text(translated);
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ background: transparent; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; overflow: hidden; }}
.card {{
  background: rgba(26, 27, 38, 0.92);
  border: 1px solid rgba(59, 66, 97, 0.6);
  border-radius: 8px;
  padding: 10px 14px;
  color: #c0caf5;
  font-size: 13px;
  line-height: 1.5;
  user-select: text;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
  animation: fadeIn 0.15s ease-out;
}}
@keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(-4px); }} to {{ opacity: 1; transform: translateY(0); }} }}
</style>
</head>
<body>
<div class="card">{escaped}</div>
<script>
// Auto-dismiss after configured timeout
setTimeout(() => window.__TAURI__?.core.invoke('close_overlay'), {dismiss_ms});
// Click to dismiss
document.addEventListener('click', () => window.__TAURI__?.core.invoke('close_overlay'));
document.addEventListener('keydown', e => {{ if (e.key === 'Escape') window.__TAURI__?.core.invoke('close_overlay'); }});
</script>
</body>
</html>"#
    )
}

/// L2: Standard overlay - source + translated, copy button
fn create_l2_overlay(source: &str, translated: &str) -> String {
    let src_escaped = html_escape::encode_text(source);
    let trans_escaped = html_escape::encode_text(translated);
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ background: transparent; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; overflow: hidden; }}
.card {{
  background: rgba(26, 27, 38, 0.95);
  border: 1px solid rgba(59, 66, 97, 0.8);
  border-radius: 10px;
  padding: 10px 14px;
  color: #c0caf5;
  font-size: 13px;
  line-height: 1.5;
  user-select: text;
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.35);
  animation: fadeIn 0.15s ease-out;
  max-width: 400px;
}}
.source {{ color: #565f89; font-size: 12px; margin-bottom: 6px; max-height: 60px; overflow: hidden; text-overflow: ellipsis; }}
.translated {{ color: #c0caf5; }}
.actions {{ display: flex; gap: 4px; margin-top: 8px; justify-content: flex-end; }}
.btn {{
  background: rgba(59, 66, 97, 0.5);
  border: 1px solid rgba(59, 66, 97, 0.8);
  color: #a9b1d6;
  border-radius: 6px;
  padding: 3px 10px;
  font-size: 11px;
  cursor: pointer;
}}
.btn:hover {{ background: rgba(122, 162, 247, 0.2); border-color: #7aa2f7; }}
.btn.done {{ background: rgba(158, 206, 106, 0.2); border-color: #9ece6a; color: #9ece6a; }}
@keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(-4px); }} to {{ opacity: 1; transform: translateY(0); }} }}
</style>
</head>
<body>
<div class="card">
  <div class="source">{src_escaped}</div>
  <div class="translated">{trans_escaped}</div>
  <div class="actions">
    <button class="btn" id="copyBtn">Copy</button>
    <button class="btn" id="closeBtn">Close</button>
  </div>
</div>
<script>
const trans = document.querySelector('.translated').textContent;
document.getElementById('copyBtn').onclick = async () => {{
  await navigator.clipboard.writeText(trans);
  const btn = document.getElementById('copyBtn');
  btn.textContent = 'Copied!'; btn.classList.add('done');
  setTimeout(() => {{ btn.textContent = 'Copy'; btn.classList.remove('done'); }}, 1500);
}};
document.getElementById('closeBtn').onclick = () => window.__TAURI__?.core.invoke('close_overlay');
document.addEventListener('keydown', e => {{ if (e.key === 'Escape') window.__TAURI__?.core.invoke('close_overlay'); }});
</script>
</body>
</html>"#
    )
}

/// L3: Full overlay - source + translated, all controls
fn create_l3_overlay(source: &str, translated: &str) -> String {
    let src_escaped = html_escape::encode_text(source);
    let trans_escaped = html_escape::encode_text(translated);
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ background: transparent; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; overflow: hidden; }}
.card {{
  background: rgba(26, 27, 38, 0.95);
  border: 1px solid rgba(59, 66, 97, 0.8);
  border-radius: 10px;
  padding: 12px 16px;
  color: #c0caf5;
  font-size: 14px;
  line-height: 1.6;
  user-select: text;
  pointer-events: auto;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  animation: fadeIn 0.15s ease-out;
  max-width: 450px;
}}
.header {{
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgba(59, 66, 97, 0.5);
}}
.title {{ font-size: 11px; color: #7aa2f7; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; }}
.actions {{ display: flex; gap: 4px; }}
.btn {{
  background: rgba(59, 66, 97, 0.5);
  border: 1px solid rgba(59, 66, 97, 0.8);
  color: #a9b1d6;
  border-radius: 6px;
  padding: 4px 10px;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s ease;
}}
.btn:hover {{ background: rgba(122, 162, 247, 0.2); border-color: #7aa2f7; color: #c0caf5; }}
.btn-close:hover {{ background: rgba(247, 118, 142, 0.2); border-color: #f7768e; color: #f7768e; }}
.btn-copy.done {{ background: rgba(158, 206, 106, 0.2); border-color: #9ece6a; color: #9ece6a; }}
.btn-pin.active {{ background: rgba(249, 226, 175, 0.2); border-color: #f9e2af; color: #f9e2af; }}
.btn-passthrough.active {{ background: rgba(137, 180, 250, 0.2); border-color: #89b4fa; color: #89b4fa; }}
.source {{ color: #565f89; font-size: 12px; margin-bottom: 8px; padding-bottom: 8px; border-bottom: 1px solid rgba(59, 66, 97, 0.3); }}
.translated {{ white-space: pre-wrap; word-break: break-word; }}
@keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(-4px); }} to {{ opacity: 1; transform: translateY(0); }} }}
</style>
</head>
<body>
<div class="card">
  <div class="header">
    <span class="title">Translation</span>
    <div class="actions">
      <button class="btn btn-pin" id="pinBtn" title="Pin">📌</button>
      <button class="btn btn-passthrough" id="passthroughBtn" title="Click Through">👆</button>
      <button class="btn btn-copy" id="copyBtn">Copy</button>
      <button class="btn btn-close" id="closeBtn">Close</button>
    </div>
  </div>
  <div class="source">{src_escaped}</div>
  <div class="translated">{trans_escaped}</div>
</div>
<script>
const trans = document.querySelector('.translated').textContent;
document.getElementById('copyBtn').onclick = async () => {{
  await navigator.clipboard.writeText(trans);
  const btn = document.getElementById('copyBtn');
  btn.textContent = 'Copied!'; btn.classList.add('done');
  setTimeout(() => {{ btn.textContent = 'Copy'; btn.classList.remove('done'); }}, 1500);
}};
const pinBtn = document.getElementById('pinBtn');
pinBtn.classList.add('active'); // starts pinned
pinBtn.onclick = async () => {{
  const pinned = await window.__TAURI__?.core.invoke('pin_overlay');
  if (pinned) {{ pinBtn.classList.add('active'); }}
  else {{ pinBtn.classList.remove('active'); }}
}};
const passthroughBtn = document.getElementById('passthroughBtn');
passthroughBtn.onclick = async () => {{
  const active = !passthroughBtn.classList.contains('active');
  await window.__TAURI__?.core.invoke('set_overlay_click_through', {{ ignore: active }});
  if (active) {{ passthroughBtn.classList.add('active'); }}
  else {{ passthroughBtn.classList.remove('active'); }}
}};
// Listen for click-through disabled event from global shortcut
window.__TAURI__?.event.listen('overlay-click-through-off', () => {{
  passthroughBtn.classList.remove('active');
}});
document.getElementById('closeBtn').onclick = () => window.__TAURI__?.core.invoke('close_overlay');
document.addEventListener('keydown', e => {{ if (e.key === 'Escape') window.__TAURI__?.core.invoke('close_overlay'); }});
</script>
</body>
</html>"#
    )
}

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
    use tauri::WebviewUrl;
    use tauri::WebviewWindowBuilder;

    // Close existing overlay if any
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.close();
    }

    let level = if show_controls.unwrap_or(false) { 3 } else { 1 };
    let html = create_overlay_html("", &text, level, 3000);
    let encoded = urlencoding::encode(&html);
    let overlay_url = format!("data:text/html,{}", encoded);

    WebviewWindowBuilder::new(&app, "overlay", WebviewUrl::App(overlay_url.into()))
        .title("Translation")
        .inner_size(width.max(200.0), height.max(50.0))
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(true)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[command]
pub async fn close_overlay(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        window.close().map_err(|e| e.to_string())?;
    }
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

    // Get config for target language and overlay settings
    let config = state.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
    let config_level = config.overlay_level;
    let dismiss_ms = config.overlay_auto_dismiss_ms;
    drop(config);

    // Use TranslationService for the full pipeline (glossary, blacklist, cache, history, metrics)
    let response = state.translation_service.translate(&text, &from, &to).await?;

    if let Some(first) = response.results.first() {
        // Get mouse position for overlay placement
        let (cursor_x, cursor_y) = get_cursor_position().await.unwrap_or((100.0, 100.0));
        let overlay_x = cursor_x + 10.0;
        let overlay_y = cursor_y + 10.0;
        let overlay_width = 350.0_f64;
        let overlay_height = 200.0_f64;

        // Close existing overlay
        if let Some(window) = app.get_webview_window("overlay") {
            let _ = window.close();
        }

        let level = overlay_level.unwrap_or(config_level);
        let html = create_overlay_html(&text, &first.text, level, dismiss_ms);
        let encoded = urlencoding::encode(&html);
        let overlay_url = format!("data:text/html,{}", encoded);

        use tauri::WebviewUrl;
        use tauri::WebviewWindowBuilder;

        WebviewWindowBuilder::new(&app, "overlay", WebviewUrl::App(overlay_url.into()))
            .title("Translation")
            .inner_size(overlay_width.max(200.0), overlay_height.max(50.0))
            .position(overlay_x, overlay_y)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(true)
            .build()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[command]
pub async fn set_overlay_click_through(app: tauri::AppHandle, ignore: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        window.set_ignore_cursor_events(ignore).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[command]
pub async fn pin_overlay(app: tauri::AppHandle) -> Result<bool, String> {
    static OVERLAY_PINNED: AtomicBool = AtomicBool::new(true); // overlay starts pinned (always_on_top)
    if let Some(window) = app.get_webview_window("overlay") {
        let current = OVERLAY_PINNED.load(Ordering::Relaxed);
        let new_value = !current;
        window.set_always_on_top(new_value).map_err(|e| e.to_string())?;
        OVERLAY_PINNED.store(new_value, Ordering::Relaxed);
        Ok(new_value)
    } else {
        Err("Overlay not found".to_string())
    }
}

#[command]
pub async fn move_overlay(app: tauri::AppHandle, x: f64, y: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(x as i32, y as i32)))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[command]
pub async fn resize_overlay(app: tauri::AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(width as u32, height as u32)))
            .map_err(|e| e.to_string())?;
    }
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

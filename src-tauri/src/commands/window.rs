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

fn create_overlay_html(text: &str, show_controls: bool) -> String {
    let escaped = html_escape::encode_text(text);
    let controls = if show_controls {
        r#"<button class="btn btn-pin" id="pinBtn" title="Pin">📌</button>
      <button class="btn btn-passthrough" id="passthroughBtn" title="Click Through">👆</button>"#
    } else {
        ""
    };
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
  background: transparent;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  overflow: hidden;
}}
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
}}
.header {{
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgba(59, 66, 97, 0.5);
}}
.title {{
  font-size: 11px;
  color: #7aa2f7;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}}
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
.btn:hover {{
  background: rgba(122, 162, 247, 0.2);
  border-color: #7aa2f7;
  color: #c0caf5;
}}
.btn-close:hover {{
  background: rgba(247, 118, 142, 0.2);
  border-color: #f7768e;
  color: #f7768e;
}}
.btn-copy.done {{
  background: rgba(158, 206, 106, 0.2);
  border-color: #9ece6a;
  color: #9ece6a;
}}
.btn-pin.active {{
  background: rgba(249, 226, 175, 0.2);
  border-color: #f9e2af;
  color: #f9e2af;
}}
.btn-passthrough.active {{
  background: rgba(137, 180, 250, 0.2);
  border-color: #89b4fa;
  color: #89b4fa;
}}
.content {{
  white-space: pre-wrap;
  word-break: break-word;
}}
.content::selection {{ background: rgba(122, 162, 247, 0.3); }}
@keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(-4px); }} to {{ opacity: 1; transform: translateY(0); }} }}
.card {{ animation: fadeIn 0.15s ease-out; }}
</style>
</head>
<body>
<div class="card">
  <div class="header">
    <span class="title">Translation</span>
    <div class="actions">
      {controls}
      <button class="btn btn-copy" id="copyBtn">Copy</button>
      <button class="btn btn-close" id="closeBtn">Close</button>
    </div>
  </div>
  <div class="content" id="text">{escaped}</div>
</div>
<script>
const text = document.getElementById('text').textContent;
document.getElementById('copyBtn').onclick = async () => {{
  try {{
    await navigator.clipboard.writeText(text);
    const btn = document.getElementById('copyBtn');
    btn.textContent = 'Copied!';
    btn.classList.add('done');
    setTimeout(() => {{ btn.textContent = 'Copy'; btn.classList.remove('done'); }}, 1500);
  }} catch(e) {{ console.error(e); }}
}};

const pinBtn = document.getElementById('pinBtn');
if (pinBtn) {{
  pinBtn.onclick = () => {{
    pinBtn.classList.toggle('active');
  }};
}}

const passthroughBtn = document.getElementById('passthroughBtn');
if (passthroughBtn) {{
  let isPassthrough = false;
  passthroughBtn.onclick = async () => {{
    isPassthrough = !isPassthrough;
    passthroughBtn.classList.toggle('active');
    await window.__TAURI__?.core.invoke('set_overlay_click_through', {{ ignore: isPassthrough }});
  }};
}}

document.getElementById('closeBtn').onclick = () => {{
  window.__TAURI__?.core.invoke('close_overlay');
}};
document.addEventListener('keydown', e => {{ if (e.key === 'Escape') window.__TAURI__?.core.invoke('close_overlay'); }});
</script>
</body>
</html>"#,
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

    let html = create_overlay_html(&text, show_controls.unwrap_or(false));
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
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Text is empty".to_string());
    }

    // Get config for target language
    let config = state.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
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

        let html = create_overlay_html(&first.text, true);
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
pub async fn pin_overlay(app: tauri::AppHandle) -> Result<(), String> {
    // Make overlay pinnable - stays on top and can be configured
    if let Some(window) = app.get_webview_window("overlay") {
        window.set_always_on_top(true).map_err(|e| e.to_string())?;
    }
    Ok(())
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

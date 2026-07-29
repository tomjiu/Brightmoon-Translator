//! Floating pop button after text selection (Easydict-style).
//! Created invisible, positioned in physical pixels, then shown once — no (0,0) flash.
//! Click via Win32 hit-test (data-URI webviews lack reliable Tauri IPC).

use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "selection-pop";
const AUTO_DISMISS_MS: u64 = 5000; // Easydict AutoDismissMs
const BTN_W: f64 = 28.0;
const BTN_H: f64 = 28.0;

struct Pending {
    text: String,
    shown_at: Instant,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn pop_html() -> String {
    r##"<!DOCTYPE html>
<html><head><meta charset="UTF-8">
<style>
html,body{margin:0;width:100%;height:100%;background:#12141a;overflow:hidden;user-select:none;}
.chip{
  width:100%;height:100%;display:flex;align-items:center;justify-content:center;
  border-radius:8px;background:#1a1d27;color:#e8eaed;
  font:600 12px/1 "Segoe UI","Microsoft YaHei",sans-serif;
  border:1px solid rgba(255,255,255,0.14);
  cursor:pointer;
}
.chip:hover{background:#2563eb;color:#fff;}
</style></head>
<body><div class="chip" id="b">译</div></body></html>
"##
    .to_string()
}

pub fn show(app: &AppHandle, text: String, screen_x: f64, screen_y: f64) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }

    {
        let mut g = PENDING.lock().map_err(|e| e.to_string())?;
        *g = Some(Pending {
            text,
            shown_at: Instant::now(),
        });
    }

    let (clamped_x, clamped_y) = crate::overlay::positioner::clamp_rect_to_cursor_monitor(
        screen_x + 8.0,
        screen_y + 8.0,
        BTN_W,
        BTN_H,
        screen_x,
        screen_y,
    );
    let x = clamped_x.max(0.0) as i32;
    let y = clamped_y.max(0.0) as i32;

    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
        let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            x, y,
        )));
        let _ = w.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            BTN_W as u32,
            BTN_H as u32,
        )));
        let _ = w.show();
        #[cfg(windows)]
        {
            if let Ok(hwnd) = w.hwnd() {
                let h = hwnd.0 as isize;
                super::win_noactivate::apply_no_activate(h);
                super::mouse_hook::set_pop_hwnd(h);
            }
            // Prefer actual outer bounds after show (DPI-correct)
            let (rx, ry, rw, rh) = if let (Ok(pos), Ok(size)) = (w.outer_position(), w.outer_size())
            {
                (pos.x, pos.y, size.width as i32, size.height as i32)
            } else {
                (x, y, BTN_W as i32, BTN_H as i32)
            };
            super::mouse_hook::set_pop_rect(rx, ry, rw.max(1), rh.max(1));
        }
        return Ok(());
    }

    let html = pop_html();
    let encoded = urlencoding::encode(&html);
    let url_str = format!("data:text/html,{}", encoded);
    let url = tauri::Url::parse(&url_str).map_err(|e| e.to_string())?;

    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title("pop")
        .inner_size(BTN_W, BTN_H)
        .decorations(false)
        .transparent(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .background_color(tauri::window::Color(18, 20, 26, 255))
        .build()
        .map_err(|e| e.to_string())?;

    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x, y,
    )));
    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
        BTN_W as u32,
        BTN_H as u32,
    )));
    let _ = window.show();
    #[cfg(windows)]
    {
        if let Ok(hwnd) = window.hwnd() {
            let h = hwnd.0 as isize;
            super::win_noactivate::apply_no_activate(h);
            super::mouse_hook::set_pop_hwnd(h);
        }
        let (rx, ry, rw, rh) =
            if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
                (pos.x, pos.y, size.width as i32, size.height as i32)
            } else {
                (x, y, BTN_W as i32, BTN_H as i32)
            };
        super::mouse_hook::set_pop_rect(rx, ry, rw.max(1), rh.max(1));
    }

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(AUTO_DISMISS_MS)).await;
        let expired = PENDING
            .lock()
            .ok()
            .and_then(|g| {
                g.as_ref()
                    .map(|p| p.shown_at.elapsed() >= Duration::from_millis(AUTO_DISMISS_MS - 50))
            })
            .unwrap_or(false);
        if expired {
            let _ = dismiss(&app2);
        }
    });

    Ok(())
}

pub fn dismiss(app: &AppHandle) -> Result<(), String> {
    if let Ok(mut g) = PENDING.lock() {
        *g = None;
    }
    #[cfg(windows)]
    super::mouse_hook::clear_pop_hwnd();
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
    Ok(())
}

pub fn take_pending() -> Option<String> {
    PENDING
        .lock()
        .ok()
        .and_then(|mut g| g.take().map(|p| p.text))
}

pub fn has_pending() -> bool {
    PENDING.lock().ok().map(|g| g.is_some()).unwrap_or(false)
}

pub fn is_pop_hwnd(app: &AppHandle, hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    app.get_webview_window(LABEL)
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize == hwnd)
        .unwrap_or(false)
}

pub fn hit_test(app: &AppHandle, screen_x: f64, screen_y: f64) -> bool {
    let Some(w) = app.get_webview_window(LABEL) else {
        return false;
    };
    if !w.is_visible().unwrap_or(false) {
        return false;
    }
    let Ok(pos) = w.outer_position() else {
        return false;
    };
    let Ok(size) = w.outer_size() else {
        return false;
    };
    let x = pos.x as f64;
    let y = pos.y as f64;
    let bw = size.width as f64;
    let bh = size.height as f64;
    screen_x >= x && screen_x <= x + bw && screen_y >= y && screen_y <= y + bh
}

//! Floating pop button after text selection (Easydict-style).
//! Created hidden off-screen (preloaded at startup), positioned in physical
//! pixels, then shown once — no (0,0) flash.
//! Click via Win32 hit-test (the FE chip never handles clicks).
//!
//! Render: React App-URL window (`index.html?window=selection-pop`), the same
//! proven path as the translate card / OCR frames. The legacy `data:text/html`
//! webview used here sometimes failed to paint and showed a plain black block.
//! The App-URL window paints the chip reliably and self-reports readiness via
//! the `POPREADY` document.title + `selection-pop-ready` event, so Rust can
//! (a) wait before the first show and (b) log if the webview never renders.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "selection-pop";
const POP_URL: &str = "index.html?window=selection-pop&v=1";
// P1 (Fix 14): 5s was too short for hesitant users; 28px too small at high DPI.
const AUTO_DISMISS_MS: u64 = 8000; // Easydict AutoDismissMs
const BTN_W: f64 = 32.0;
const BTN_H: f64 = 32.0;

struct Pending {
    text: String,
    shown_at: Instant,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
/// True once the FE webview has mounted the pop chip (set by `selection-pop-ready`).
static POP_READY: AtomicBool = AtomicBool::new(false);

/// Mark the FE mounted (from the `selection-pop-ready` event handler).
pub fn mark_pop_ready() {
    POP_READY.store(true, Ordering::SeqCst);
}

/// True once the FE webview is mounted, so the chip paints immediately on show.
pub fn pop_ready() -> bool {
    POP_READY.load(Ordering::SeqCst)
}

/// Window raw background (visible only at the chip's rounded corners).
fn theme_bg_color() -> tauri::window::Color {
    if crate::overlay::window_manager::overlay_theme_is_light() {
        tauri::window::Color(245, 245, 247, 255)
    } else {
        tauri::window::Color(18, 20, 26, 255)
    }
}

/// Create the hidden off-screen pop window if missing (safe to call repeatedly).
/// Preloaded at startup so the first `show()` is instant and already painted.
pub fn preload_selection_pop(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(LABEL).is_some() {
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(POP_URL.into()))
        .title("pop")
        .inner_size(BTN_W, BTN_H)
        // Off-screen so the hidden window never paints over the desktop if DWM
        // races the `visible(false)` setting during creation.
        .position(-32000.0, -32000.0)
        .decorations(false)
        .transparent(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .background_color(theme_bg_color())
        .build()
        .map_err(|e| e.to_string())?;
    // Force-hide after build (same DWM race that affects `visible(false)`).
    let _ = window.hide();
    tracing::info!("[pop] window created (hidden, off-screen)");
    Ok(())
}

/// Hide → position → show the existing pop window and re-assert no-activate.
fn show_existing(app: &AppHandle, x: i32, y: i32) {
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
    }
}

pub fn show(app: &AppHandle, text: String, screen_x: f64, screen_y: f64) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }
    // R2: never arm pop for chrome/junk (caller should filter; belt-and-suspenders).
    if !super::present::accept_for_pop(&text) {
        tracing::info!(
            "[pop] show rejected junk preview={:?}",
            text.chars().take(40).collect::<String>()
        );
        return Ok(());
    }

    {
        let mut g = PENDING.lock().map_err(|e| e.to_string())?;
        *g = Some(Pending {
            text: text.clone(),
            shown_at: Instant::now(),
        });
    }
    tracing::info!(
        "[pop] armed pending_len={} preview={:?}",
        text.chars().count(),
        text.chars().take(40).collect::<String>()
    );

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

    // P0 fix (Fix 4): reschedule auto_dismiss on EVERY show (reuse included).
    // Without this, only the FIRST show() spawns a dismiss task; selecting again
    // within the window updated PENDING.shown_at but no new task existed →
    // the pop button could stay forever after the first timer elapsed.
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

    // Reuse path: window exists (preloaded or created earlier) AND the FE is
    // already mounted, so hide/position/show synchronously — instant, painted.
    // If the webview is still loading (e.g. warmup-created moments ago), fall
    // through so the first show still waits for the chip to be mounted.
    if app.get_webview_window(LABEL).is_some() && pop_ready() {
        show_existing(app, x, y);
        return Ok(());
    }

    // Cold path: build hidden, then wait (bounded) for the FE to mount so the
    // first show paints the chip instead of a blank block, then show.
    preload_selection_pop(app)?;
    let armed_at = PENDING
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|p| p.shown_at));
    let app3 = app.clone();
    tauri::async_runtime::spawn(async move {
        for _ in 0..50 {
            if pop_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        // The user may have clicked elsewhere / made a newer selection while the
        // FE was still mounting — never show a stale button then. Only show if
        // PENDING still holds THIS arm (same shown_at, not dismissed, not aged).
        let current = PENDING
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|p| p.shown_at));
        let still_current = match (armed_at, current) {
            (Some(a), Some(cur)) => {
                a == cur && cur.elapsed() < Duration::from_millis(AUTO_DISMISS_MS)
            },
            _ => false,
        };
        if !still_current {
            return;
        }
        show_existing(&app3, x, y);
        // Diagnostic: confirm the FE actually painted the chip.
        if let Some(w) = app3.get_webview_window(LABEL) {
            tokio::time::sleep(Duration::from_millis(600)).await;
            match w.title() {
                Ok(title) if title == "POPREADY" => {
                    tracing::info!("[pop] FE painted (POPREADY)");
                },
                Ok(title) => {
                    tracing::warn!(
                        "[pop] FE title={:?} — pop webview may not have painted",
                        title
                    );
                },
                Err(_) => {
                    tracing::warn!("[pop] could not read title (webview missing?)");
                },
            }
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
        .and_then(|mut g| {
            g.take().map(|p| {
                // Prove show == take: same preview as the `armed pending` log in show().
                // Confirms Pop consumes the exact text captured at gesture time (no re-fetch),
                // and the age stays inside the 5000ms auto-dismiss window.
                tracing::info!(
                    "[pop] consumed pending_len={} preview={:?} age_ms={}",
                    p.text.chars().count(),
                    p.text.chars().take(40).collect::<String>(),
                    p.shown_at.elapsed().as_millis()
                );
                p.text
            })
        })
}

pub fn has_pending() -> bool {
    PENDING.lock().ok().is_some_and(|g| g.is_some())
}

pub fn is_pop_hwnd(app: &AppHandle, hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    app.get_webview_window(LABEL)
        .and_then(|w| w.hwnd().ok())
        .is_some_and(|h| h.0 as isize == hwnd)
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
    let x = f64::from(pos.x);
    let y = f64::from(pos.y);
    let bw = f64::from(size.width);
    let bh = f64::from(size.height);
    screen_x >= x && screen_x <= x + bw && screen_y >= y && screen_y <= y + bh
}

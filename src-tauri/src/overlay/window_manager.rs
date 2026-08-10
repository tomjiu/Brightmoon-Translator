use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

// S1-3: `html_builder`, `OverlayContent`, `OverlayLevel` were only used by the
// removed `create_overlay_window_via_http` and `update_overlay_content_via_shell`
// dead functions. Both were zero-caller dead code, so the imports are gone too.

/// FE theme for overlay cards (false = dark, true = light).
static OVERLAY_LIGHT: AtomicBool = AtomicBool::new(false);

pub fn set_overlay_theme_light(light: bool) {
    OVERLAY_LIGHT.store(light, Ordering::SeqCst);
}

pub fn overlay_theme_is_light() -> bool {
    OVERLAY_LIGHT.load(Ordering::SeqCst)
}

/// Shared sizing for machine-translate overlay cards.
/// Both `present::present_mt_card` and `DefaultSelectionTranslation::show_overlay`
/// use this so card dimensions stay consistent regardless of trigger path.
/// CJK-aware: characters >= U+3000 are ~15px wide, others ~8px (matches `dict_card_size`).
pub fn estimate_mt_card_size(display_text: &str) -> (f64, f64) {
    // Match build_card_html metrics: padding 22px + optional source ~24px +
    // each rendered line ~24px (13px*1.5lh + 4px mt). The display text is the
    // FULL translation (often wraps to 2-3x the line count), so the height is
    // deliberately ~2x the naive estimate — an underestimate clips the result
    // and defeats the whole card. The JS fit script only ever grows further.
    let line_n = display_text.lines().count().max(1) as f64;
    let h = (110.0 + line_n * 52.0).clamp(168.0, 720.0);
    let longest = display_text
        .lines()
        .map(|l| {
            l.chars()
                .map(|c| if c >= '\u{3000}' { 15.0_f64 } else { 8.0 })
                .sum::<f64>()
        })
        .fold(0.0_f64, f64::max);
    let w = (longest + 48.0).clamp(200.0, 460.0);
    (w, h)
}

/// Close existing overlay and create a new one.
/// x, y are in physical pixels (from Win32 APIs).
///
/// This function tries to reuse an existing overlay window first.
/// If the overlay window already exists, it updates content via `eval()`
/// instead of destroying and recreating.
pub fn create_overlay_window(
    app: &AppHandle,
    html: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    always_on_top: bool,
) -> Result<(), String> {
    create_overlay_window_ex(app, html, x, y, width, height, always_on_top, false)
}

/// `steal_focus`: false for hover/dict so typing is not interrupted.
pub fn create_overlay_window_ex(
    app: &AppHandle,
    html: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    always_on_top: bool,
    steal_focus: bool,
) -> Result<(), String> {
    // Tight bounds — oversized empty window looked like "blank lower half".
    // Raised the height cap (420→720) so long machine-translation cards fit
    // without clipping (the JS fit script grows it further up to 720). The
    // monitor clamp keeps them on-screen.
    let w = width.clamp(120.0, 620.0);
    let h = height.clamp(48.0, 720.0);
    // Multi-monitor: keep card on the monitor under the placement point (QTranslate)
    let (cx, cy) = crate::overlay::positioner::clamp_rect_to_cursor_monitor(x, y, w, h, x, y);
    let px = cx.max(0.0) as i32;
    let py = cy.max(0.0) as i32;

    let encoded = urlencoding::encode(html);
    let overlay_url_str = format!("data:text/html,{encoded}");
    let overlay_url = tauri::Url::parse(&overlay_url_str)
        .map_err(|e| format!("Failed to parse overlay URL: {e}"))?;

    // Reuse: hide → move/size → navigate to fresh data URL → show.
    // P0 (Fix C): navigation (not `document.documentElement.innerHTML = <full
    // <html> doc>`) — injecting a complete document into documentElement nests
    // <html>/<head>/<body>, breaking .card rendering into an empty dark blob,
    // and the hide→eval→show async race crashes WebView2 (Error 1412,
    // ERROR_HOOK_NEEDS_HMOD at Chrome_WidgetWin unregister). A real navigation
    // replaces the document cleanly (no nesting, no eval race, no doc.write
    // memory leak) with no visible corner flash.
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            px, py,
        )));
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            w as u32, h as u32,
        )));
        let _ = window.set_always_on_top(always_on_top);
        match window.navigate(overlay_url.clone()) {
            Ok(()) => {
                tracing::info!(
                    "[overlay] reused window -> navigated {}x{} @ ({},{})",
                    w,
                    h,
                    px,
                    py
                );
            },
            Err(e) => tracing::error!("[overlay] navigate failed: {e}"),
        }
        let _ = window.show();
        #[cfg(windows)]
        if let Ok(hwnd) = window.hwnd() {
            crate::selection::win_noactivate::apply_no_activate(hwnd.0 as isize);
        }
        if steal_focus {
            let _ = window.set_focus();
        }
        note_overlay_shown();
        // MOONDIAG: poll the webview document.title ~600ms after show; the card
        // JS writes diagnostic values there so we can see (without IPC) whether
        // inline scripts ran and what they measured. Log ANY title so we can
        // tell "script never ran" (stays "Translation") from "invoke missing".
        let probe_window = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            if let Ok(title) = probe_window.title() {
                if title.starts_with("MOONDIAG") || title.starts_with("DIAG") {
                    tracing::info!("[overlay] JS DIAG title={title}");
                } else {
                    tracing::info!("[overlay] JS probe title={title} (script did not run?)");
                }
            }
        });
        return Ok(());
    }

    // Create invisible off-cursor, then physical move, then show once.
    let window = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::External(overlay_url))
        .title("Translation")
        .inner_size(w, h)
        .decorations(false)
        .transparent(false)
        .always_on_top(always_on_top)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .background_color(if overlay_theme_is_light() {
            tauri::window::Color(255, 255, 255, 255)
        } else {
            tauri::window::Color(26, 26, 30, 255)
        })
        .build()
        .map_err(|e| e.to_string())?;

    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        px, py,
    )));
    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
        w as u32, h as u32,
    )));
    let _ = window.show();
    tracing::info!(
        "[overlay] created new window {}x{} @ ({},{}) light={}",
        w,
        h,
        px,
        py,
        overlay_theme_is_light()
    );
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        crate::selection::win_noactivate::apply_no_activate(hwnd.0 as isize);
    }
    if steal_focus {
        let _ = window.set_focus();
    }
    note_overlay_shown();

    Ok(())
}

static OVERLAY_SHOWN_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

fn note_overlay_shown() {
    if let Ok(mut g) = OVERLAY_SHOWN_AT.lock() {
        *g = Some(std::time::Instant::now());
    }
}

/// True if overlay was shown less than `ms` ago (grace before mouse-leave dismiss).
pub fn overlay_shown_within_ms(ms: u64) -> bool {
    OVERLAY_SHOWN_AT
        .lock()
        .ok()
        .and_then(|g| g.map(|t| t.elapsed().as_millis() < u128::from(ms)))
        .unwrap_or(false)
}

/// Screen bounds of the overlay window if visible (for mouse-leave dismiss).
pub fn overlay_screen_bounds(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let w = app.get_webview_window("overlay")?;
    if !w.is_visible().ok()? {
        return None;
    }
    let pos = w.outer_position().ok()?;
    let size = w.outer_size().ok()?;
    Some((
        f64::from(pos.x),
        f64::from(pos.y),
        f64::from(size.width),
        f64::from(size.height),
    ))
}

/// Close/hide the overlay. Prefer hide to avoid recreate flash at (0,0).
pub fn close_overlay_window(app: &AppHandle) {
    hide_overlay_window(app);
}

/// Hide the overlay window instead of closing it (for window pooling)
pub fn hide_overlay_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
    if let Ok(mut g) = OVERLAY_SHOWN_AT.lock() {
        *g = None;
    }
    // S2-5: schedule a deferred destroy. Hidden overlay webviews still hold
    // memory (DOM + JS heap + WebView2 resources). If the overlay is not
    // reshown within 5 minutes, destroy the window so the OS reclaims the
    // memory. The next create_overlay_window_ex call will recreate it.
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_mins(5)).await;
        // Only destroy if still hidden (no new show happened).
        if let Some(window) = app2.get_webview_window("overlay") {
            if !window.is_visible().unwrap_or(false) {
                tracing::info!("[overlay] destroying idle hidden overlay (5min unused) to reclaim memory");
                let _ = window.close();
            }
        }
    });
}

/// Move overlay to a new position
pub fn move_overlay_window(app: &AppHandle, x: f64, y: f64) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            x as i32, y as i32,
        )));
    }
}

/// Resize overlay
pub fn resize_overlay_window(app: &AppHandle, width: f64, height: f64) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            width as u32,
            height as u32,
        )));
    }
}

/// Update overlay content in-place without rebuilding the window.
/// This preserves pin/click-through state.
pub fn update_overlay_content(
    app: &AppHandle,
    source: &str,
    translated: &str,
) -> Result<bool, String> {
    let window = match app.get_webview_window("overlay") {
        Some(w) => w,
        None => return Ok(false),
    };

    // Escape for JS string literals
    let src_escaped = source
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('$', "\\$");
    let trans_escaped = translated
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('$', "\\$");

    // Update DOM elements if they exist, using data attributes for reliable selection
    let js = format!(
        r#"
        (function() {{
            const srcEl = document.querySelector('[data-role="source"]');
            const transEl = document.querySelector('[data-role="translated"]');
            if (srcEl) srcEl.textContent = `{src_escaped}`;
            if (transEl) transEl.textContent = `{trans_escaped}`;
        }})();
        "#
    );

    window.eval(&js).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Update overlay position without rebuilding
pub fn update_overlay_position(app: &AppHandle, x: f64, y: f64) -> Result<bool, String> {
    let window = match app.get_webview_window("overlay") {
        Some(w) => w,
        None => return Ok(false),
    };

    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            x as i32, y as i32,
        )))
        .map_err(|e| e.to_string())?;

    Ok(true)
}

/// Check if overlay window exists
pub fn overlay_exists(app: &AppHandle) -> bool {
    app.get_webview_window("overlay").is_some()
}

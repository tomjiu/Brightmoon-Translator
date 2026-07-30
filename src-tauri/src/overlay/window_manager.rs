use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use super::html_builder;
use super::{OverlayContent, OverlayLevel};

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
pub fn estimate_mt_card_size(display_text: &str) -> (f64, f64) {
    let line_n = display_text.lines().count().max(1) as f64;
    let h = (56.0 + line_n * 22.0).clamp(72.0, 320.0);
    let w = (display_text
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(16) as f64
        * 8.0
        + 48.0)
        .clamp(200.0, 460.0);
    (w, h)
}

/// Close existing overlay and create a new one.
/// x, y are in physical pixels (from Win32 APIs).
///
/// This function tries to reuse an existing overlay window first.
/// If the overlay window already exists, it updates content via eval()
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
    // Tight bounds — oversized empty window looked like "blank lower half"
    let w = width.clamp(120.0, 480.0);
    let h = height.clamp(48.0, 320.0);
    // Multi-monitor: keep card on the monitor under the placement point (QTranslate)
    let (cx, cy) = crate::overlay::positioner::clamp_rect_to_cursor_monitor(x, y, w, h, x, y);
    let px = cx.max(0.0) as i32;
    let py = cy.max(0.0) as i32;

    // Reuse: hide → move/size → write content → show (no destroy, no corner flash)
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            px, py,
        )));
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            w as u32, h as u32,
        )));
        let _ = window.set_always_on_top(always_on_top);

        let escaped = html
            .replace('\\', "\\\\")
            .replace('`', "\\`")
            .replace('$', "\\$");
        let js = format!(
            r#"
            document.open();
            document.write(`{}`);
            document.close();
            "#,
            escaped
        );
        let _ = window.eval(&js);
        let _ = window.show();
        #[cfg(windows)]
        if let Ok(hwnd) = window.hwnd() {
            crate::selection::win_noactivate::apply_no_activate(hwnd.0 as isize);
        }
        if steal_focus {
            let _ = window.set_focus();
        }
        note_overlay_shown();
        return Ok(());
    }

    let encoded = urlencoding::encode(html);
    let overlay_url_str = format!("data:text/html,{}", encoded);
    let overlay_url = tauri::Url::parse(&overlay_url_str)
        .map_err(|e| format!("Failed to parse overlay URL: {}", e))?;

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
        .and_then(|g| g.map(|t| t.elapsed().as_millis() < ms as u128))
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
        pos.x as f64,
        pos.y as f64,
        size.width as f64,
        size.height as f64,
    ))
}

/// Create overlay window using the HTTP server for content delivery.
/// This is the optimized path that avoids data URI encoding overhead.
pub fn create_overlay_window_via_http(
    app: &AppHandle,
    http_base_url: &str,
    content: &OverlayContent,
    level: OverlayLevel,
    dismiss_ms: u64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    always_on_top: bool,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            x.max(0.0) as i32,
            y.max(0.0) as i32,
        )));
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            width.max(200.0) as u32,
            height.max(50.0) as u32,
        )));
        let _ = window.set_always_on_top(always_on_top);
        let js = html_builder::build_update_script(
            &content.source,
            &content.translated,
            level,
            dismiss_ms,
        );
        let _ = window.eval(&js);
        let _ = window.show();
        note_overlay_shown();
        return Ok(());
    }

    // Create invisible first — never flash at (0,0)
    let overlay_url_str = format!("{}/overlay", http_base_url);
    let overlay_url = tauri::Url::parse(&overlay_url_str)
        .map_err(|e| format!("Failed to parse overlay URL: {}", e))?;

    let w = width.max(200.0);
    let h = height.max(50.0);
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
        .background_color(tauri::window::Color(18, 18, 20, 255))
        .build()
        .map_err(|e| e.to_string())?;

    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x.max(0.0) as i32,
        y.max(0.0) as i32,
    )));
    let _ = window.show();
    note_overlay_shown();

    // Give the webview a moment to load the shell, then update with actual content
    let content_clone = content.clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Some(w) = app_clone.get_webview_window("overlay") {
            let js = html_builder::build_update_script(
                &content_clone.source,
                &content_clone.translated,
                level,
                dismiss_ms,
            );
            let _ = w.eval(&js);
        }
    });

    Ok(())
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
}

/// Show a previously hidden overlay window (no focus steal — terminal multi-window safe)
pub fn show_overlay_window(app: &AppHandle) -> bool {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.show();
        #[cfg(windows)]
        if let Ok(hwnd) = window.hwnd() {
            crate::selection::win_noactivate::apply_no_activate(hwnd.0 as isize);
        }
        return true;
    }
    false
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

/// Update overlay content using the RAF-based shell update mechanism
pub fn update_overlay_content_via_shell(
    app: &AppHandle,
    source: &str,
    translated: &str,
    level: OverlayLevel,
    dismiss_ms: u64,
) -> Result<bool, String> {
    let window = match app.get_webview_window("overlay") {
        Some(w) => w,
        None => return Ok(false),
    };

    let js = html_builder::build_update_script(source, translated, level, dismiss_ms);
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

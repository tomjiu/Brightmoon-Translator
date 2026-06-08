use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use super::html_builder;
use super::{OverlayContent, OverlayLevel};

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
    // Try to reuse existing overlay window
    if let Some(window) = app.get_webview_window("overlay") {
        // Window exists - just update position and show it
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            x as i32, y as i32,
        )));
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            width.max(200.0) as u32,
            height.max(50.0) as u32,
        )));
        let _ = window.set_always_on_top(always_on_top);
        let _ = window.show();
        let _ = window.set_focus();

        // Update content via eval (in-place DOM update, preserves pin/click-through state)
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
        return Ok(());
    }

    // No existing window - create a new one using data URI (fallback)
    let encoded = urlencoding::encode(html);
    let overlay_url_str = format!("data:text/html,{}", encoded);
    let overlay_url = tauri::Url::parse(&overlay_url_str)
        .map_err(|e| format!("Failed to parse overlay URL: {}", e))?;

    // Create window at origin first (position() expects logical coords).
    // We use position(0,0) then move to physical coords to avoid DPI mismatch.
    let window = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::External(overlay_url))
        .title("Translation")
        .inner_size(width.max(200.0), height.max(50.0))
        .position(0.0, 0.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(always_on_top)
        .skip_taskbar(true)
        .resizable(true)
        .focused(false)
        .build()
        .map_err(|e| e.to_string())?;

    // Move to the correct physical position
    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x as i32, y as i32,
    )));

    Ok(())
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
    // Try to reuse existing overlay window
    if let Some(window) = app.get_webview_window("overlay") {
        // Window exists - update position, resize, and update content via eval
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            x as i32, y as i32,
        )));
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            width.max(200.0) as u32,
            height.max(50.0) as u32,
        )));
        let _ = window.set_always_on_top(always_on_top);
        let _ = window.show();
        let _ = window.set_focus();

        // Update content via the shell's __overlayUpdate function (RAF-based)
        let js = html_builder::build_update_script(
            &content.source,
            &content.translated,
            level,
            dismiss_ms,
        );
        let _ = window.eval(&js);
        return Ok(());
    }

    // No existing window - create a new one using HTTP URL
    let overlay_url_str = format!("{}/overlay", http_base_url);
    let overlay_url = tauri::Url::parse(&overlay_url_str)
        .map_err(|e| format!("Failed to parse overlay URL: {}", e))?;

    let window = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::External(overlay_url))
        .title("Translation")
        .inner_size(width.max(200.0), height.max(50.0))
        .position(0.0, 0.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(always_on_top)
        .skip_taskbar(true)
        .resizable(true)
        .focused(false)
        .build()
        .map_err(|e| e.to_string())?;

    // Move to the correct physical position
    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x as i32, y as i32,
    )));

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

/// Close the overlay window if it exists
pub fn close_overlay_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.close();
    }
}

/// Hide the overlay window instead of closing it (for window pooling)
pub fn hide_overlay_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
}

/// Show a previously hidden overlay window
pub fn show_overlay_window(app: &AppHandle) -> bool {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.show();
        let _ = window.set_focus();
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

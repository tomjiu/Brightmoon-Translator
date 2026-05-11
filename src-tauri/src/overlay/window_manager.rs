use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Close existing overlay and create a new one.
/// x, y are in physical pixels (from Win32 APIs).
pub fn create_overlay_window(
    app: &AppHandle,
    html: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    always_on_top: bool,
) -> Result<(), String> {
    // Close existing overlay if any
    close_overlay_window(app);

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

/// Close the overlay window if it exists
pub fn close_overlay_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.close();
    }
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
/// Returns true if overlay existed and was updated, false if overlay doesn't exist.
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

/// Update overlay position without rebuilding.
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

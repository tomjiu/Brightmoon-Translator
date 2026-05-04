use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Close existing overlay and create a new one
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
    let overlay_url = format!("data:text/html,{}", encoded);

    WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App(overlay_url.into()))
        .title("Translation")
        .inner_size(width.max(200.0), height.max(50.0))
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .always_on_top(always_on_top)
        .skip_taskbar(true)
        .resizable(false)
        .focused(true)
        .build()
        .map_err(|e| e.to_string())?;

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
            width as u32, height as u32,
        )));
    }
}

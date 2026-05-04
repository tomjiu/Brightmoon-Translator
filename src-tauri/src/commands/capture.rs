use base64::Engine;
use screenshots::Screen;
use std::io::Cursor;
use tauri::command;

fn image_to_base64_png(image: &screenshots::image::DynamicImage) -> Result<String, String> {
    let mut buf = Cursor::new(Vec::new());
    image
        .write_to(&mut buf, screenshots::image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    Ok(format!("data:image/png;base64,{}", base64_str))
}

#[command]
pub async fn capture_screen(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;

    let screen = screens
        .first()
        .ok_or_else(|| "No screen found".to_string())?;

    // Capture the specified region
    let buffer = screen
        .capture_area(x, y, width, height)
        .map_err(|e| format!("Failed to capture area: {}", e))?;

    // Convert to DynamicImage
    let img = screenshots::image::DynamicImage::ImageRgba8(buffer);

    image_to_base64_png(&img)
}

#[command]
pub async fn capture_full_screen() -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;

    let screen = screens
        .first()
        .ok_or_else(|| "No screen found".to_string())?;

    let buffer = screen.capture().map_err(|e| format!("Failed to capture screen: {}", e))?;

    let img = screenshots::image::DynamicImage::ImageRgba8(buffer);

    image_to_base64_png(&img)
}

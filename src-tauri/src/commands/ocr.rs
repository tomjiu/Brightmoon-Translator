use base64::Engine;
use serde::Serialize;
use tauri::command;

#[derive(Serialize)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f64,
}

#[command]
pub async fn ocr_screen(image_base64: String) -> Result<OcrResult, String> {
    // Decode base64 image
    let _image_data = base64::engine::general_purpose::STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    // For now, return a placeholder - Windows OCR integration will be added
    // when the Windows crate is available
    let text = String::from("[OCR功能待实现]");

    Ok(OcrResult {
        text,
        confidence: 0.0,
    })
}

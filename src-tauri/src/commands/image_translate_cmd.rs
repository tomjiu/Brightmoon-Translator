//! Tauri commands for image translation.

use crate::image_translate::{ImagePreview, ImageTranslationResult};
use crate::security;
use crate::AppState;
use tauri::{command, State};

/// Translate an image file: OCR -> translate -> overlay translated text.
///
/// Reads the image at `input_path`, runs OCR, translates detected text,
/// and writes the translated image to `output_path`.
#[command]
pub async fn translate_image(
    state: State<'_, AppState>,
    input_path: String,
    output_path: String,
    from_lang: String,
    to_lang: String,
    ocr_engine: Option<String>,
    app_key: Option<String>,
    app_secret: Option<String>,
) -> Result<ImageTranslationResult, String> {
    security::validate_file_path(&input_path)?;
    security::validate_output_path(&output_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;

    tracing::info!(
        "[translate_image] {} -> {}, {} -> {}",
        input_path,
        output_path,
        from_lang,
        to_lang
    );

    let engine_type = ocr_engine.unwrap_or_else(|| "winrt".to_string());

    let translation_service = state.translation.service.clone();

    // Read Youdao OCR keys from config if not provided
    let config = state.system.config.lock().await;
    let effective_app_key = app_key
        .filter(|k| !k.is_empty())
        .or_else(|| Some(config.engines.youdao.ocr_app_key.clone()));
    let effective_app_secret = app_secret
        .filter(|s| !s.is_empty())
        .or_else(|| Some(config.engines.youdao.ocr_app_secret.clone()));
    drop(config);

    crate::image_translate::translate_image_file(
        &input_path,
        &output_path,
        &from_lang,
        &to_lang,
        &engine_type,
        translation_service,
        effective_app_key,
        effective_app_secret,
    )
    .await
}

/// Preview OCR results on an image file without translating.
/// Returns detected text lines with bounding boxes.
#[command]
pub async fn preview_image_translation(
    state: State<'_, AppState>,
    input_path: String,
    lang: String,
    ocr_engine: Option<String>,
    app_key: Option<String>,
    app_secret: Option<String>,
) -> Result<ImagePreview, String> {
    security::validate_file_path(&input_path)?;
    security::validate_language_code(&lang)?;

    tracing::info!("[preview_image_translation] {}, lang={}", input_path, lang);

    let engine_type = ocr_engine.unwrap_or_else(|| "winrt".to_string());

    // Read Youdao OCR keys from config if not provided
    let config = state.system.config.lock().await;
    let effective_app_key = app_key
        .filter(|k| !k.is_empty())
        .or_else(|| Some(config.engines.youdao.ocr_app_key.clone()));
    let effective_app_secret = app_secret
        .filter(|s| !s.is_empty())
        .or_else(|| Some(config.engines.youdao.ocr_app_secret.clone()));
    drop(config);

    crate::image_translate::preview_image_ocr(
        &input_path,
        &lang,
        &engine_type,
        effective_app_key,
        effective_app_secret,
    )
    .await
}

/// Translate an image from base64 data (for in-memory images).
/// Returns the translated image as base64 PNG.
#[command]
pub async fn translate_image_base64(
    state: State<'_, AppState>,
    base64_data: String,
    from_lang: String,
    to_lang: String,
    ocr_engine: Option<String>,
    app_key: Option<String>,
    app_secret: Option<String>,
) -> Result<TranslatedImageBase64, String> {
    tracing::info!(
        "[translate_image_base64] {} -> {}, data size={} chars",
        from_lang,
        to_lang,
        base64_data.len()
    );

    // Decode base64 image
    let b64 = base64_data
        .strip_prefix("data:image/")
        .and_then(|s| s.find(',').map(|i| &s[i + 1..]))
        .unwrap_or(&base64_data);

    let image_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;

    // Save to temp file for processing
    let temp_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir();
    let input_path = temp_dir.join(format!("moontranslator_img_input_{}.png", temp_id));
    let output_path = temp_dir.join(format!("moontranslator_img_output_{}.png", temp_id));

    std::fs::write(&input_path, &image_bytes)
        .map_err(|e| format!("Failed to write temp file: {}", e))?;

    let engine_type = ocr_engine.unwrap_or_else(|| "winrt".to_string());
    let translation_service = state.translation.service.clone();

    // Read Youdao OCR keys from config if not provided
    let config = state.system.config.lock().await;
    let effective_app_key = app_key
        .filter(|k| !k.is_empty())
        .or_else(|| Some(config.engines.youdao.ocr_app_key.clone()));
    let effective_app_secret = app_secret
        .filter(|s| !s.is_empty())
        .or_else(|| Some(config.engines.youdao.ocr_app_secret.clone()));
    drop(config);

    let result = crate::image_translate::translate_image_file(
        input_path.to_str().unwrap_or_default(),
        output_path.to_str().unwrap_or_default(),
        &from_lang,
        &to_lang,
        &engine_type,
        translation_service,
        effective_app_key,
        effective_app_secret,
    )
    .await;

    // Clean up input temp file
    let _ = std::fs::remove_file(&input_path);

    match result {
        Ok(info) => {
            // Read output image and encode to base64
            let output_bytes = std::fs::read(&output_path)
                .map_err(|e| format!("Failed to read output image: {}", e))?;
            let _ = std::fs::remove_file(&output_path);

            let base64_output = format!(
                "data:image/png;base64,{}",
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &output_bytes)
            );

            Ok(TranslatedImageBase64 {
                image: base64_output,
                lines_translated: info.lines_translated,
                total_lines: info.total_lines,
                width: info.original_width,
                height: info.original_height,
            })
        },
        Err(e) => {
            let _ = std::fs::remove_file(&output_path);
            Err(e)
        },
    }
}

/// Result of base64 image translation.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedImageBase64 {
    pub image: String,
    pub lines_translated: usize,
    pub total_lines: usize,
    pub width: u32,
    pub height: u32,
}

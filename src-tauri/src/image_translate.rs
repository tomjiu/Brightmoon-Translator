//! Image translation module: OCR recognition -> translate -> overlay translated text.
//!
//! Workflow:
//! 1. Read source image
//! 2. Run OCR with bounding boxes (WinRT or Youdao)
//! 3. Translate each detected text line
//! 4. Draw translated text over original text positions
//! 5. Output the translated image

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use serde::Serialize;

use crate::commands::capture::OcrResultDetailed;
use crate::services::TranslationService;
use std::sync::Arc;

/// Result of an image translation operation.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageTranslationResult {
    pub output_path: String,
    pub lines_translated: usize,
    pub total_lines: usize,
    pub original_width: u32,
    pub original_height: u32,
}

/// Result of image OCR preview (before translation).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePreview {
    pub width: u32,
    pub height: u32,
    pub lines: Vec<PreviewLine>,
    pub full_text: String,
}

/// A single line in the image preview.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLine {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Configuration for image translation rendering.
struct RenderConfig {
    /// Background color for text area (RGBA)
    bg_color: Rgba<u8>,
    /// Text color (RGBA)
    text_color: Rgba<u8>,
    /// Padding around text in pixels
    padding: u32,
    /// Whether to sample background color from the image
    sample_bg: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            bg_color: Rgba([255, 255, 255, 230]),
            text_color: Rgba([0, 0, 0, 255]),
            padding: 2,
            sample_bg: true,
        }
    }
}

/// Find a system font that supports the target language.
/// Tries common CJK and Latin fonts on Windows.
fn find_system_font(target_lang: &str) -> Option<Vec<u8>> {
    let font_dir = if cfg!(target_os = "windows") {
        std::path::PathBuf::from("C:\\Windows\\Fonts")
    } else {
        dirs::font_dir().unwrap_or_default()
    };

    // Select font candidates based on target language
    let candidates: Vec<&str> = match target_lang {
        "zh" | "zh-CN" | "zh-CHS" | "zh-TW" | "zh-CHT" => vec![
            "msyh.ttc",   // Microsoft YaHei
            "msyhbd.ttc", // Microsoft YaHei Bold
            "simhei.ttf", // SimHei
            "simsun.ttc", // SimSun
            "msyh.ttf",   // Microsoft YaHei (older)
        ],
        "ja" => vec![
            "msgothic.ttc", // MS Gothic
            "msmincho.ttc", // MS Mincho
            "YuGothR.ttc",  // Yu Gothic
            "msyh.ttc",     // Microsoft YaHei (fallback)
        ],
        "ko" => vec![
            "malgun.ttf", // Malgun Gothic
            "batang.ttc", // Batang
            "msyh.ttc",   // Fallback
        ],
        _ => vec![
            "arial.ttf",   // Arial
            "segoeui.ttf", // Segoe UI
            "tahoma.ttf",  // Tahoma
            "verdana.ttf", // Verdana
        ],
    };

    for font_name in &candidates {
        let font_path = font_dir.join(font_name);
        if font_path.exists() {
            if let Ok(data) = std::fs::read(&font_path) {
                tracing::info!("[ImageTranslate] Loaded font: {:?}", font_path);
                return Some(data);
            }
        }
    }

    // Try to load any available font from the font directory
    if let Ok(entries) = std::fs::read_dir(&font_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "ttf" || ext == "ttc" || ext == "otf" {
                if let Ok(data) = std::fs::read(&path) {
                    tracing::info!("[ImageTranslate] Fallback font: {:?}", path);
                    return Some(data);
                }
            }
        }
    }

    None
}

/// Sample the dominant background color from the edges of a text region.
fn sample_background_color(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> Rgba<u8> {
    let (img_w, img_h) = img.dimensions();
    if img_w == 0 || img_h == 0 {
        return Rgba([255, 255, 255, 255]);
    }

    let mut r_sum: u64 = 0;
    let mut g_sum: u64 = 0;
    let mut b_sum: u64 = 0;
    let mut count: u64 = 0;

    // Sample from top and bottom edges (2px strip)
    let sample_h = 2u32.min(h);
    for dx in 0..w.min(img_w.saturating_sub(x)) {
        for dy in 0..sample_h {
            // Top edge
            let px = x + dx;
            let py = y + dy;
            if px < img_w && py < img_h {
                let p = img.get_pixel(px, py);
                r_sum += p[0] as u64;
                g_sum += p[1] as u64;
                b_sum += p[2] as u64;
                count += 1;
            }
            // Bottom edge
            let py2 = y + h - 1 - dy;
            if px < img_w && py2 < img_h {
                let p = img.get_pixel(px, py2);
                r_sum += p[0] as u64;
                g_sum += p[1] as u64;
                b_sum += p[2] as u64;
                count += 1;
            }
        }
    }

    // Sample from left and right edges (2px strip)
    let sample_w = 2u32.min(w);
    for dy in 0..h.min(img_h.saturating_sub(y)) {
        for dx in 0..sample_w {
            // Left edge
            let px = x + dx;
            let py = y + dy;
            if px < img_w && py < img_h {
                let p = img.get_pixel(px, py);
                r_sum += p[0] as u64;
                g_sum += p[1] as u64;
                b_sum += p[2] as u64;
                count += 1;
            }
            // Right edge
            let px2 = x + w - 1 - dx;
            if px2 < img_w && py < img_h {
                let p = img.get_pixel(px2, py);
                r_sum += p[0] as u64;
                g_sum += p[1] as u64;
                b_sum += p[2] as u64;
                count += 1;
            }
        }
    }

    if count == 0 {
        return Rgba([255, 255, 255, 255]);
    }

    Rgba([
        (r_sum / count) as u8,
        (g_sum / count) as u8,
        (b_sum / count) as u8,
        255,
    ])
}

/// Calculate the optimal font size to fit translated text within a bounding box.
/// Returns (font_size, wrapped_lines).
fn calculate_font_size_and_wrap(
    text: &str,
    max_width: u32,
    max_height: u32,
    font: &FontRef<'_>,
) -> (f32, Vec<String>) {
    // Start with a font size based on the bounding box height
    let line_count = text.lines().count().max(1);
    let mut font_size = (max_height as f32 / line_count as f32 * 0.8).clamp(10.0, 72.0);

    // Reduce font size until text fits within the bounding box
    loop {
        if font_size < 8.0 {
            font_size = 8.0;
            break;
        }

        let wrapped = wrap_text(text, max_width, font_size, font);
        let total_height = wrapped.len() as f32 * font_size * 1.3;

        if total_height <= max_height as f32 || font_size <= 8.0 {
            return (font_size, wrapped);
        }

        font_size -= 1.0;
    }

    let wrapped = wrap_text(text, max_width, font_size, font);
    (font_size, wrapped)
}

/// Wrap text to fit within a given pixel width at a given font size.
fn wrap_text(text: &str, max_width: u32, font_size: f32, font: &FontRef<'_>) -> Vec<String> {
    let scale = PxScale::from(font_size);
    let mut lines = Vec::new();

    for line in text.lines() {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        let chars: Vec<char> = line.chars().collect();

        for ch in chars {
            let test_line = if current_line.is_empty() {
                ch.to_string()
            } else {
                format!("{}{}", current_line, ch)
            };

            let width = measure_text_width(&test_line, font, scale);
            if width > max_width as f32 && !current_line.is_empty() {
                lines.push(current_line.clone());
                current_line = ch.to_string();
            } else {
                current_line = test_line;
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    if lines.is_empty() {
        lines.push(text.to_string());
    }

    lines
}

/// Measure the pixel width of a text string.
fn measure_text_width(text: &str, font: &FontRef<'_>, scale: PxScale) -> f32 {
    let mut width = 0.0f32;
    let mut prev_glyph = None;

    for ch in text.chars() {
        let glyph_id = font.glyph_id(ch);
        width += font
            .as_scaled(scale)
            .kern(prev_glyph.unwrap_or(glyph_id), glyph_id);
        width += font.as_scaled(scale).h_advance(glyph_id);
        prev_glyph = Some(glyph_id);
    }

    width
}

/// Draw a filled rectangle with alpha blending.
fn draw_filled_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    let (img_w, img_h) = img.dimensions();
    let alpha = color[3] as f32 / 255.0;

    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < img_w && py < img_h {
                let existing = img.get_pixel(px, py);
                let blended = Rgba([
                    (color[0] as f32 * alpha + existing[0] as f32 * (1.0 - alpha)) as u8,
                    (color[1] as f32 * alpha + existing[1] as f32 * (1.0 - alpha)) as u8,
                    (color[2] as f32 * alpha + existing[2] as f32 * (1.0 - alpha)) as u8,
                    255,
                ]);
                img.put_pixel(px, py, blended);
            }
        }
    }
}

/// Draw a single line of text centered within a bounding box.
fn draw_line_in_box(
    img: &mut RgbaImage,
    text: &str,
    box_x: u32,
    box_y: u32,
    box_w: u32,
    box_h: u32,
    font: &FontRef<'_>,
    font_size: f32,
    text_color: Rgba<u8>,
) {
    let scale = PxScale::from(font_size);

    // Calculate text dimensions for centering
    let text_width = measure_text_width(text, font, scale);
    let text_height = font_size;

    // Center text within the bounding box
    let x_offset = if text_width < box_w as f32 {
        box_x + ((box_w as f32 - text_width) / 2.0) as u32
    } else {
        box_x
    };

    let y_offset = if text_height < box_h as f32 {
        box_y + ((box_h as f32 - text_height) / 2.0) as u32
    } else {
        box_y
    };

    draw_text_mut(
        img,
        text_color,
        x_offset as i32,
        y_offset as i32,
        scale,
        font,
        text,
    );
}

/// Translate an image: OCR -> translate -> overlay translated text.
///
/// `ocr_engine_type`: "winrt" for Windows native OCR, "youdao" for Youdao OCR.
pub async fn translate_image_file(
    input_path: &str,
    output_path: &str,
    from_lang: &str,
    to_lang: &str,
    ocr_engine_type: &str,
    translation_service: Arc<TranslationService>,
    app_key: Option<String>,
    app_secret: Option<String>,
) -> Result<ImageTranslationResult, String> {
    tracing::info!(
        "[ImageTranslate] Starting: {} -> {}, lang {} -> {}",
        input_path,
        output_path,
        from_lang,
        to_lang
    );

    // 1. Load image
    let img = image::open(input_path).map_err(|e| format!("Failed to open image: {}", e))?;
    let (img_width, img_height) = img.dimensions();
    let mut result_img = img.to_rgba8();

    // 2. Run OCR
    let ocr_result =
        run_image_ocr(input_path, ocr_engine_type, from_lang, app_key, app_secret).await?;

    if ocr_result.lines.is_empty() {
        return Err("OCR did not detect any text in the image".to_string());
    }

    tracing::info!(
        "[ImageTranslate] OCR detected {} lines",
        ocr_result.lines.len()
    );

    // 3. Find and load font
    let font_data = find_system_font(to_lang)
        .ok_or_else(|| "No suitable font found for the target language".to_string())?;
    let font =
        FontRef::try_from_slice(&font_data).map_err(|e| format!("Failed to load font: {}", e))?;

    let config = RenderConfig::default();

    // 4. Translate and render each line
    let mut translated_count = 0usize;
    let total_lines = ocr_result.lines.len();

    for line in &ocr_result.lines {
        if line.text.trim().is_empty() {
            continue;
        }

        // Translate the line
        let translated = match translation_service
            .run_primary(
                crate::models::translation::TranslateChannel::Image,
                line.text.trim(),
                from_lang,
                to_lang,
            )
            .await
        {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    "[ImageTranslate] Translation failed for {:?}: {}",
                    line.text,
                    e
                );
                continue;
            },
        };

        if translated.trim().is_empty() || translated.trim() == line.text.trim() {
            continue;
        }

        // Get bounding box coordinates (clamp to image bounds)
        let bx = (line.x as u32).min(img_width.saturating_sub(1));
        let by = (line.y as u32).min(img_height.saturating_sub(1));
        let bw = (line.width as u32).min(img_width.saturating_sub(bx));
        let bh = (line.height as u32).min(img_height.saturating_sub(by));

        if bw == 0 || bh == 0 {
            continue;
        }

        // Sample background color from the image edges
        let bg_color = if config.sample_bg {
            let sampled = sample_background_color(&result_img, bx, by, bw, bh);
            // Make it slightly more opaque for better text readability
            Rgba([sampled[0], sampled[1], sampled[2], 230])
        } else {
            config.bg_color
        };

        // Draw background rectangle to cover original text
        draw_filled_rect(
            &mut result_img,
            bx.saturating_sub(config.padding),
            by.saturating_sub(config.padding),
            bw + config.padding * 2,
            bh + config.padding * 2,
            bg_color,
        );

        // Calculate font size and wrap text
        let (font_size, wrapped_lines) = calculate_font_size_and_wrap(&translated, bw, bh, &font);

        // Draw each wrapped line
        let line_height = font_size * 1.3;
        let total_text_height = wrapped_lines.len() as f32 * line_height;
        let y_start = if total_text_height < bh as f32 {
            by + ((bh as f32 - total_text_height) / 2.0) as u32
        } else {
            by
        };

        for (i, wrapped_line) in wrapped_lines.iter().enumerate() {
            let y_pos = y_start + (i as f32 * line_height) as u32;
            if y_pos + font_size as u32 > img_height {
                break;
            }
            draw_line_in_box(
                &mut result_img,
                wrapped_line,
                bx,
                y_pos,
                bw,
                font_size as u32,
                &font,
                font_size,
                config.text_color,
            );
        }

        translated_count += 1;
    }

    // 5. Save output image
    let output_img = DynamicImage::ImageRgba8(result_img);
    output_img
        .save(output_path)
        .map_err(|e| format!("Failed to save output image: {}", e))?;

    tracing::info!(
        "[ImageTranslate] Done: {}/{} lines translated",
        translated_count,
        total_lines
    );

    Ok(ImageTranslationResult {
        output_path: output_path.to_string(),
        lines_translated: translated_count,
        total_lines,
        original_width: img_width,
        original_height: img_height,
    })
}

/// Run OCR on an image file, returning detailed results with bounding boxes.
async fn run_image_ocr(
    image_path: &str,
    engine_type: &str,
    lang: &str,
    app_key: Option<String>,
    app_secret: Option<String>,
) -> Result<OcrResultDetailed, String> {
    // Load image and encode to PNG bytes for OCR
    let img =
        image::open(image_path).map_err(|e| format!("Failed to open image for OCR: {}", e))?;

    let mut png_buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png_buf, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image to PNG: {}", e))?;
    let png_bytes = png_buf.into_inner();

    // Convert to owned String for use in spawn_blocking
    let lang_owned = if lang == "auto" {
        None
    } else {
        Some(lang.to_string())
    };

    match engine_type {
        "winrt" => {
            crate::commands::capture::run_winrt_ocr_detailed_from_bytes(
                &png_bytes,
                lang_owned.as_deref(),
            )
            .await
        },
        "youdao" => {
            // Use standalone Youdao OCR with default 30s timeout
            crate::commands::capture::run_youdao_ocr_from_bytes(
                &png_bytes,
                Some(lang.to_string()),
                app_key,
                app_secret,
                30,
            )
            .await
        },
        _ => {
            // Default to WinRT OCR
            crate::commands::capture::run_winrt_ocr_detailed_from_bytes(
                &png_bytes,
                lang_owned.as_deref(),
            )
            .await
        },
    }
}

/// Preview OCR on an image file without translating.
pub async fn preview_image_ocr(
    image_path: &str,
    lang: &str,
    ocr_engine_type: &str,
    app_key: Option<String>,
    app_secret: Option<String>,
) -> Result<ImagePreview, String> {
    let img = image::open(image_path).map_err(|e| format!("Failed to open image: {}", e))?;
    let (width, height) = img.dimensions();

    let ocr_result = run_image_ocr(image_path, ocr_engine_type, lang, app_key, app_secret).await?;

    let preview_lines: Vec<PreviewLine> = ocr_result
        .lines
        .iter()
        .map(|l| PreviewLine {
            text: l.text.clone(),
            x: l.x,
            y: l.y,
            width: l.width,
            height: l.height,
        })
        .collect();

    Ok(ImagePreview {
        width,
        height,
        lines: preview_lines,
        full_text: ocr_result.text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text() {
        // Basic test - would need a real font for proper testing
        let font_data = find_system_font("en");
        if let Some(data) = font_data {
            let font = FontRef::try_from_slice(&data).unwrap();
            let lines = wrap_text("Hello World Test", 100, 16.0, &font);
            assert!(!lines.is_empty());
        }
    }
}

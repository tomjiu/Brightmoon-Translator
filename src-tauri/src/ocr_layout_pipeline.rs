//! Auto text region detect — improves OCR quality by running DocLayout-YOLO
//! layout detection before OCR, then OCR-ing only text-bearing regions
//! (title / plain_text / captions / list) while skipping non-text regions
//! (figure / table / formula / abandon / header / footer).
//!
//! ## Pipeline
//! 1. Check `layout_detection_enabled` config + model availability.
//! 2. If disabled or model missing → delegate to existing full-image OCR.
//! 3. Load the LayoutDetector and run inference on the screenshot.
//! 4. Filter detected regions: keep text classes, skip non-text classes.
//! 5. For each text region: crop → OCR → offset bounding boxes by region origin.
//! 6. Merge all per-region lines, sorted top-to-bottom by y coordinate.
//! 7. On any error in the layout path → graceful fallback to full-image OCR.
//!
//! ## Feature gating
//! The `LayoutDetector` is behind the `layout-detection` Cargo feature.
//! When the feature is OFF, this module always delegates to full-image OCR,
//! so callers can unconditionally use [`ocr_with_layout_detection`] without
//! conditional compilation on the call site.

use crate::commands::capture::{
    run_offline_ocr_detailed, run_winrt_ocr_detailed_from_bytes, OcrResultDetailed,
};
#[cfg(feature = "layout-detection")]
use crate::commands::capture::{OcrLineResult, OcrWordResult};
use crate::config::AppConfig;
use crate::layout_detection::DOC_LAYOUT_CLASSES;
#[cfg(feature = "layout-detection")]
use crate::layout_detection::{BBox, LayoutRegion};
use tauri::AppHandle;

/// Class IDs that are considered "text-bearing" and should be OCR'd.
///
/// DocLayout-YOLO classes:
/// 0: title, 1: plain_text, 2: abandon, 3: figure, 4: figure_caption,
/// 5: table, 6: table_caption, 7: table_footnote, 8: is_list,
/// 9: formula, 10: page_header, 11: page_footer
const TEXT_CLASS_IDS: &[u32] = &[
    0,  // title
    1,  // plain_text
    4,  // figure_caption
    6,  // table_caption
    7,  // table_footnote
    8,  // is_list
];

/// Returns true if the class ID represents a text-bearing region.
fn is_text_region(class_id: u32) -> bool {
    TEXT_CLASS_IDS.contains(&class_id)
}

/// Run OCR with optional layout-detection pre-processing.
///
/// When `layout_detection_enabled` is true in config AND the ONNX model is
/// available AND the `layout-detection` Cargo feature is enabled, this
/// function:
/// 1. Runs DocLayout-YOLO to detect layout regions
/// 2. Filters to text regions only (skips figure/table/formula/abandon)
/// 3. OCRs each text region separately
/// 4. Merges results with correct bounding box offsets
///
/// Otherwise (or on any error), it delegates to the existing full-image OCR
/// path (`winrt` or `offline` backend).
///
/// # Arguments
/// * `app` - Tauri app handle (for model path resolution)
/// * `png_bytes` - Raw PNG image bytes
/// * `lang` - OCR language hint (None = auto-detect)
/// * `ocr_backend` - "winrt" or "offline" (falls back to winrt on unknown)
#[cfg_attr(not(feature = "layout-detection"), allow(unused_variables))]
pub async fn ocr_with_layout_detection(
    app: &AppHandle,
    png_bytes: &[u8],
    lang: Option<&str>,
    ocr_backend: &str,
) -> Result<OcrResultDetailed, String> {
    let config = AppConfig::load();

    if !config.layout_detection_enabled {
        return ocr_full_image(png_bytes, lang, ocr_backend, &config).await;
    }

    #[cfg(feature = "layout-detection")]
    {
        return ocr_with_layout_inner(app, png_bytes, lang, ocr_backend, &config).await;
    }

    #[cfg(not(feature = "layout-detection"))]
    {
        tracing::debug!(
            "[LayoutPipeline] layout-detection feature not compiled, using full-image OCR"
        );
        return ocr_full_image(png_bytes, lang, ocr_backend, &config).await;
    }
}

/// Full-image OCR fallback (no layout detection).
async fn ocr_full_image(
    png_bytes: &[u8],
    lang: Option<&str>,
    ocr_backend: &str,
    config: &AppConfig,
) -> Result<OcrResultDetailed, String> {
    if ocr_backend == "offline" {
        let backend = config.offline_ocr.backend.clone();
        let plugin_dir = config.offline_ocr.plugin_dir.clone();
        // Dimensions are not known here; pass 0 to let the synthetic helper
        // compute from the image. The offline path uses synthetic line bands
        // anyway, so exact dimensions don't affect correctness.
        run_offline_ocr_detailed(png_bytes, &backend, &plugin_dir, lang.map(String::from), 0.0, 0.0)
            .await
    } else {
        run_winrt_ocr_detailed_from_bytes(png_bytes, lang).await
    }
}

/// Layout-detection OCR pipeline (feature-gated).
#[cfg(feature = "layout-detection")]
async fn ocr_with_layout_inner(
    app: &AppHandle,
    png_bytes: &[u8],
    lang: Option<&str>,
    ocr_backend: &str,
    config: &AppConfig,
) -> Result<OcrResultDetailed, String> {
    use crate::layout_detection::LayoutDetector;
    use crate::layout_model_download;

    // 1. Check model availability.
    if !layout_model_download::is_model_ready(app) {
        tracing::info!("[LayoutPipeline] Model not ready, falling back to full-image OCR");
        return ocr_full_image(png_bytes, lang, ocr_backend, config).await;
    }

    let model_path = layout_model_download::model_path(app);

    // 2. Load the detector.
    let detector = match LayoutDetector::load(&model_path.to_string_lossy()) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("[LayoutPipeline] Failed to load model: {e}, falling back");
            return ocr_full_image(png_bytes, lang, ocr_backend, config).await;
        }
    };

    // 3. Decode the image.
    let img = match image::load_from_memory(png_bytes) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!("[LayoutPipeline] Failed to decode image: {e}, falling back");
            return ocr_full_image(png_bytes, lang, ocr_backend, config).await;
        }
    };

    // 4. Run layout detection.
    let regions = detector.detect(&img);
    if regions.is_empty() {
        tracing::info!("[LayoutPipeline] No regions detected, falling back to full-image OCR");
        return ocr_full_image(png_bytes, lang, ocr_backend, config).await;
    }

    // 5. Filter to text regions.
    let text_regions: Vec<&LayoutRegion> = regions.iter().filter(|r| is_text_region(r.class_id)).collect();

    let skipped_count = regions.len() - text_regions.len();
    tracing::info!(
        "[LayoutPipeline] Detected {} regions ({} text, {} skipped: figure/table/formula/etc)",
        regions.len(),
        text_regions.len(),
        skipped_count
    );

    if text_regions.is_empty() {
        tracing::info!("[LayoutPipeline] No text regions found, falling back to full-image OCR");
        return ocr_full_image(png_bytes, lang, ocr_backend, config).await;
    }

    // 6. OCR each text region and merge results.
    let mut all_lines: Vec<OcrLineResult> = Vec::new();

    for region in &text_regions {
        let crop_bytes = match crop_image_to_png(&img, &region.bbox) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    "[LayoutPipeline] Failed to crop region {:?} at ({:.0},{:.0}): {e}",
                    region.class_name,
                    region.bbox.x,
                    region.bbox.y
                );
                continue;
            }
        };

        let region_result = match ocr_full_image(&crop_bytes, lang, ocr_backend, config).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "[LayoutPipeline] OCR failed for region at ({:.0},{:.0}): {e}",
                    region.bbox.x,
                    region.bbox.y
                );
                continue;
            }
        };

        // Offset bounding boxes by the region's origin so they map back to
        // the original full-image coordinate space.
        let offset_x = region.bbox.x as f64;
        let offset_y = region.bbox.y as f64;

        for line in region_result.lines {
            all_lines.push(OcrLineResult {
                text: line.text,
                x: line.x + offset_x,
                y: line.y + offset_y,
                width: line.width,
                height: line.height,
                words: line
                    .words
                    .into_iter()
                    .map(|w| OcrWordResult {
                        text: w.text,
                        x: w.x + offset_x,
                        y: w.y + offset_y,
                        width: w.width,
                        height: w.height,
                    })
                    .collect(),
            });
        }
    }

    // 7. Sort lines top-to-bottom by y coordinate (reading order).
    all_lines.sort_by(|a, b| {
        a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
    });

    // 8. Merge text using geometric post-processing (paragraph structure, CJK spacing).
    all_lines.retain(|l| !l.text.trim().is_empty());
    let full_text = crate::ocr_postprocess::join_text_regions(&all_lines);

    tracing::info!(
        "[LayoutPipeline] Merged {} lines from {} regions",
        all_lines.len(),
        text_regions.len()
    );

    Ok(OcrResultDetailed {
        text: full_text,
        lines: all_lines,
    })
}

/// Crop a rectangular region from an image and encode it as PNG bytes.
#[cfg(feature = "layout-detection")]
fn crop_image_to_png(
    img: &image::DynamicImage,
    bbox: &BBox,
) -> Result<Vec<u8>, String> {
    let (img_w, img_h) = (img.width(), img.height());

    // Clamp the crop rect to image bounds and convert to integers.
    let x0 = (bbox.x.max(0.0) as u32).min(img_w);
    let y0 = (bbox.y.max(0.0) as u32).min(img_h);
    let x1 = ((bbox.x + bbox.width).max(0.0) as u32).min(img_w);
    let y1 = ((bbox.y + bbox.height).max(0.0) as u32).min(img_h);

    let crop_w = x1.saturating_sub(x0);
    let crop_h = y1.saturating_sub(y0);

    if crop_w == 0 || crop_h == 0 {
        return Err(format!(
            "Degenerate crop rect: ({},{}) {}x{} (image {}x{})",
            x0, y0, crop_w, crop_h, img_w, img_h
        ));
    }

    let cropped = img.crop_imm(x0, y0, crop_w, crop_h);

    let mut buf = std::io::Cursor::new(Vec::new());
    cropped
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode crop as PNG: {e}"))?;
    Ok(buf.into_inner())
}

/// Human-readable summary of which classes are text vs non-text.
/// Used for diagnostics and logging.
pub fn classify_region(class_id: u32) -> &'static str {
    if is_text_region(class_id) {
        "text"
    } else {
        "non-text"
    }
}

/// Get the class name for a given class ID.
pub fn class_name(class_id: u32) -> &'static str {
    DOC_LAYOUT_CLASSES
        .get(class_id as usize)
        .copied()
        .unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_class_ids_are_valid() {
        for &id in TEXT_CLASS_IDS {
            assert!(id < DOC_LAYOUT_CLASSES.len() as u32);
        }
    }

    #[test]
    fn is_text_region_correct() {
        assert!(is_text_region(0)); // title
        assert!(is_text_region(1)); // plain_text
        assert!(is_text_region(4)); // figure_caption
        assert!(is_text_region(8)); // is_list
        assert!(!is_text_region(2)); // abandon
        assert!(!is_text_region(3)); // figure
        assert!(!is_text_region(5)); // table
        assert!(!is_text_region(9)); // formula
        assert!(!is_text_region(10)); // page_header
        assert!(!is_text_region(11)); // page_footer
    }

    #[test]
    fn classify_region_text_vs_non_text() {
        assert_eq!(classify_region(0), "text");
        assert_eq!(classify_region(1), "text");
        assert_eq!(classify_region(3), "non-text");
        assert_eq!(classify_region(9), "non-text");
    }

    #[test]
    fn class_name_lookup() {
        assert_eq!(class_name(0), "title");
        assert_eq!(class_name(1), "plain_text");
        assert_eq!(class_name(3), "figure");
        assert_eq!(class_name(9), "formula");
        assert_eq!(class_name(99), "unknown");
    }
}

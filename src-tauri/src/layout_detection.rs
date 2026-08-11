//! P6: Document layout detection via DocLayout-YOLO.
//!
//! Detects layout regions (text, title, figure, table, etc.) in document
//! page images to improve translation quality for complex layouts. Uses
//! an ONNX-trained YOLO model; the model file and ONNX Runtime shared
//! library are loaded at runtime (not bundled) to keep the binary small.
//!
//! ## Architecture
//! - **Image preprocessing** (pure Rust): resize to model input size,
//!   normalize to [0, 1], convert to NCHW tensor layout.
//! - **ONNX inference** (feature-gated behind `layout-detection`): runs the
//!   DocLayout-YOLO model to produce raw detection tensors.
//! - **YOLO post-processing** (pure Rust): decode boxes, filter by
//!   confidence, apply Non-Maximum Suppression (NMS).
//!
//! ## Model
//! The user provides a DocLayout-YOLO `.onnx` model path in config. The
//! model is typically ~50 MB and is NOT bundled with the app. Download
//! from the DocLayout-YOLO `GitHub` releases.
//!
//! ## Classes (DocLayout-YOLO standard)
//! 0: title, 1: plain text, 2: abandon, 3: figure, 4: `figure_caption`,
//! 5: table, 6: `table_caption`, 7: `table_footnote`, 8: `is_list`,
//! 9: formula, 10: `page_header`, 11: `page_footer`

use serde::{Deserialize, Serialize};

/// Default confidence threshold for keeping detections.
pub const DEFAULT_CONF_THRESHOLD: f32 = 0.25;
/// Default `IoU` threshold for Non-Maximum Suppression.
pub const DEFAULT_NMS_IOU_THRESHOLD: f32 = 0.45;
/// Default model input size (DocLayout-YOLO uses 1024×1024).
pub const DEFAULT_MODEL_INPUT_SIZE: u32 = 1024;

/// DocLayout-YOLO class labels (index → name).
pub const DOC_LAYOUT_CLASSES: &[&str] = &[
    "title",
    "plain_text",
    "abandon",
    "figure",
    "figure_caption",
    "table",
    "table_caption",
    "table_footnote",
    "is_list",
    "formula",
    "page_header",
    "page_footer",
];

/// A detected layout region in a document page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRegion {
    /// Class index (0-11, see `DOC_LAYOUT_CLASSES`).
    pub class_id: u32,
    /// Human-readable class name.
    pub class_name: String,
    /// Confidence score [0, 1].
    pub confidence: f32,
    /// Bounding box in original image pixel coordinates.
    pub bbox: BBox,
}

/// Axis-aligned bounding box in pixel coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BBox {
    /// Left edge (x1).
    pub x: f32,
    /// Top edge (y1).
    pub y: f32,
    /// Right edge (x2).
    pub width: f32,
    /// Bottom edge (y2).
    pub height: f32,
}

impl BBox {
    /// Create from corner coordinates (x1, y1, x2, y2).
    pub fn from_corners(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        }
    }

    /// Right edge (x2).
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge (y2).
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Area (width × height). Returns 0 for degenerate boxes.
    pub fn area(&self) -> f32 {
        self.width.max(0.0) * self.height.max(0.0)
    }

    /// Intersection-over-Union (`IoU`) with another box.
    pub fn iou(&self, other: &BBox) -> f32 {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = self.right().min(other.right());
        let y2 = self.bottom().min(other.bottom());
        let inter_w = (x2 - x1).max(0.0);
        let inter_h = (y2 - y1).max(0.0);
        let inter = inter_w * inter_h;
        let union = self.area() + other.area() - inter;
        if union <= 0.0 {
            0.0
        } else {
            inter / union
        }
    }
}

/// Decode a single YOLO detection from raw model output.
///
/// YOLOv8/v10 output format per anchor: [cx, cy, w, h, `class_0_score`,
/// `class_1_score`, ...]. The box is in model-input pixel coordinates and
/// needs to be scaled back to original image coordinates.
///
/// Returns the best (highest-confidence) class and its detection.
fn decode_yolo_anchor(
    anchor: &[f32],
    num_classes: usize,
    conf_threshold: f32,
    scale_x: f32,
    scale_y: f32,
) -> Option<LayoutRegion> {
    if anchor.len() < 4 + num_classes {
        return None;
    }
    let cx = anchor[0];
    let cy = anchor[1];
    let w = anchor[2];
    let h = anchor[3];

    // Find the best class.
    let mut best_class = 0u32;
    let mut best_score = 0.0f32;
    for i in 0..num_classes {
        let score = anchor[4 + i];
        if score > best_score {
            best_score = score;
            best_class = i as u32;
        }
    }

    if best_score < conf_threshold {
        return None;
    }

    // Convert center-format to corner-format and scale to original image.
    let x1 = (cx - w / 2.0) * scale_x;
    let y1 = (cy - h / 2.0) * scale_y;
    let x2 = (cx + w / 2.0) * scale_x;
    let y2 = (cy + h / 2.0) * scale_y;

    Some(LayoutRegion {
        class_id: best_class,
        class_name: DOC_LAYOUT_CLASSES
            .get(best_class as usize)
            .unwrap_or(&"unknown")
            .to_string(),
        confidence: best_score,
        bbox: BBox::from_corners(x1, y1, x2, y2),
    })
}

/// Apply Non-Maximum Suppression to a list of detections.
///
/// Removes overlapping boxes of the same class, keeping the highest-
/// confidence one. Boxes with `IoU` > `iou_threshold` are considered
/// duplicates.
pub fn non_max_suppression(
    detections: Vec<LayoutRegion>,
    iou_threshold: f32,
) -> Vec<LayoutRegion> {
    if detections.is_empty() {
        return detections;
    }

    // Sort by confidence descending.
    let mut sorted = detections;
    sorted.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut kept: Vec<LayoutRegion> = Vec::with_capacity(sorted.len());
    let mut suppressed = vec![false; sorted.len()];

    for i in 0..sorted.len() {
        if suppressed[i] {
            continue;
        }
        kept.push(sorted[i].clone());
        for j in (i + 1)..sorted.len() {
            if suppressed[j] {
                continue;
            }
            // Only suppress same-class overlaps.
            if sorted[i].class_id != sorted[j].class_id {
                continue;
            }
            let iou = sorted[i].bbox.iou(&sorted[j].bbox);
            if iou > iou_threshold {
                suppressed[j] = true;
            }
        }
    }

    kept
}

/// Post-process raw YOLO model output into layout regions.
///
/// `raw_output` is the flattened detection tensor in shape
/// `[1, num_anchors, 4 + num_classes]` (YOLOv8/v10 format). The function
/// decodes each anchor, filters by confidence, applies NMS, and returns
/// the final detections in original image coordinates.
///
/// `scale_x`/`scale_y` convert from model-input coordinates to original
/// image coordinates (`original_width` / `model_input_width`, etc.).
pub fn post_process_yolo(
    raw_output: &[f32],
    num_anchors: usize,
    num_classes: usize,
    conf_threshold: f32,
    iou_threshold: f32,
    scale_x: f32,
    scale_y: f32,
) -> Vec<LayoutRegion> {
    let stride = 4 + num_classes;
    let mut detections: Vec<LayoutRegion> = Vec::new();

    for i in 0..num_anchors {
        let offset = i * stride;
        if offset + stride > raw_output.len() {
            break;
        }
        let anchor = &raw_output[offset..offset + stride];
        if let Some(det) =
            decode_yolo_anchor(anchor, num_classes, conf_threshold, scale_x, scale_y)
        {
            detections.push(det);
        }
    }

    non_max_suppression(detections, iou_threshold)
}

/// Preprocess an image for YOLO inference.
///
/// Resizes the image to `target_size × target_size` (letterbox padding to
/// preserve aspect ratio), normalizes pixels to [0, 1], and converts to
/// NCHW tensor layout (1×3×H×W).
///
/// Returns the tensor data and the scale factor used (for post-processing
/// to map boxes back to original coordinates).
#[cfg(feature = "layout-detection")]
pub fn preprocess_image(
    img: &image::DynamicImage,
    target_size: u32,
) -> (Vec<f32>, f32, f32) {
    let orig_w = img.width() as f32;
    let orig_h = img.height() as f32;

    // Letterbox: resize keeping aspect ratio, pad the rest with 114 (gray).
    let scale = (target_size as f32 / orig_w).min(target_size as f32 / orig_h);
    let new_w = (orig_w * scale).round() as u32;
    let new_h = (orig_h * scale).round() as u32;

    let resized = image::imageops::resize(
        &img.to_rgb8(),
        new_w,
        new_h,
        image::imageops::FilterType::Triangle,
    );

    // Create padded image filled with 114 (YOLO standard padding value).
    let mut padded = image::RgbImage::from_pixel(target_size, target_size, image::Rgb([114, 114, 114]));
    let pad_x = (target_size - new_w) / 2;
    let pad_y = (target_size - new_h) / 2;
    image::imageops::overlay(&mut padded, &resized, pad_x as i64, pad_y as i64);

    // Convert to NCHW tensor, normalized to [0, 1].
    let mut tensor = vec![0.0f32; (3 * target_size * target_size) as usize];
    for y in 0..target_size {
        for x in 0..target_size {
            let px = padded.get_pixel(x, y);
            let idx = (y * target_size + x) as usize;
            // R, G, B channels
            tensor[idx] = px[0] as f32 / 255.0;
            tensor[(target_size * target_size) as usize + idx] = px[1] as f32 / 255.0;
            tensor[(2 * target_size * target_size) as usize + idx] = px[2] as f32 / 255.0;
        }
    }

    // Scale factor to map model-output boxes back to original coordinates.
    // The model sees a letterboxed image, so we need to undo the padding and scaling.
    let scale_x = 1.0 / scale; // model coords → original coords
    let scale_y = 1.0 / scale;

    (tensor, scale_x, scale_y)
}

/// Layout detector that loads and runs a DocLayout-YOLO ONNX model.
///
/// The model file and ONNX Runtime shared library are loaded at runtime.
/// If either is missing, detection returns an empty vec (graceful
/// degradation — callers fall back to existing text extraction).
#[cfg(feature = "layout-detection")]
pub struct LayoutDetector {
    session: ort::session::Session,
    model_input_size: u32,
    num_classes: usize,
}

#[cfg(feature = "layout-detection")]
impl LayoutDetector {
    /// Load a DocLayout-YOLO model from an ONNX file.
    ///
    /// `model_path` is the path to the `.onnx` model file. The ONNX Runtime
    /// shared library must be available (onnxruntime.dll / libonnxruntime.so).
    pub fn load(model_path: &str) -> Result<Self, String> {
        use ort::session::Session;

        let session = Session::builder()
            .map_err(|e| format!("ONNX session builder: {e}"))?
            .with_optimization_level(ort::session::GraphOptimizationLevel::Level3)
            .map_err(|e| format!("ONNX opt level: {e}"))?
            .commit_in_file(model_path)
            .map_err(|e| format!("ONNX model load from {model_path}: {e}"))?;

        Ok(Self {
            session,
            model_input_size: DEFAULT_MODEL_INPUT_SIZE,
            num_classes: DOC_LAYOUT_CLASSES.len(),
        })
    }

    /// Detect layout regions in an image.
    ///
    /// Returns a list of detected regions sorted by confidence (descending).
    /// Returns empty vec on any inference error (graceful degradation).
    pub fn detect(&self, img: &image::DynamicImage) -> Vec<LayoutRegion> {
        let (tensor, scale_x, scale_y) = preprocess_image(img, self.model_input_size);

        let input_tensor = match ort::value::Tensor::from_array((
            tensor,
            vec![1, 3, self.model_input_size as i64, self.model_input_size as i64],
        )) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("[P6] ONNX input tensor creation failed: {}", e);
                return Vec::new();
            }
        };

        let outputs = match self.session.run(ort::inputs![input_tensor].unwrap_or_default()) {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!("[P6] ONNX inference failed: {}", e);
                return Vec::new();
            }
        };

        // YOLOv8 output: [1, 4+num_classes, num_anchors] or [1, num_anchors, 4+num_classes]
        // Extract the raw float data and post-process.
        let output = match outputs.into_iter().next() {
            Some(o) => o,
            None => {
                tracing::warn!("[P6] ONNX model produced no outputs");
                return Vec::new();
            }
        };

        let (data, shape) = match output.try_into_tensor() {
            Ok(t) => (t.data().to_vec(), t.shape().to_vec()),
            Err(e) => {
                tracing::warn!("[P6] ONNX output tensor extraction failed: {}", e);
                return Vec::new();
            }
        };

        // Determine layout: [1, A, C] or [1, C, A]
        let num_anchors = if shape.len() == 3 {
            if shape[1] > shape[2] {
                // [1, A, C] — anchors first
                shape[1] as usize
            } else {
                // [1, C, A] — need to transpose (channels first)
                shape[2] as usize
            }
        } else {
            tracing::warn!("[P6] unexpected ONNX output shape: {:?}", shape);
            return Vec::new();
        };

        // If output is [1, C, A], transpose to [1, A, C] for our decoder.
        let stride = 4 + self.num_classes;
        let decoded_data: Vec<f32> = if shape.len() == 3 && shape[1] <= shape[2] {
            // Transpose [1, C, A] → [1, A, C]
            let c = shape[1] as usize;
            let a = shape[2] as usize;
            (0..a)
                .flat_map(|ai| (0..c).map(move |ci| data[ci * a + ai]))
                .collect()
        } else {
            data
        };

        post_process_yolo(
            &decoded_data,
            num_anchors,
            self.num_classes,
            DEFAULT_CONF_THRESHOLD,
            DEFAULT_NMS_IOU_THRESHOLD,
            scale_x,
            scale_y,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_iou_identical_boxes() {
        let a = BBox::from_corners(10.0, 10.0, 50.0, 50.0);
        let b = BBox::from_corners(10.0, 10.0, 50.0, 50.0);
        assert!((a.iou(&b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn bbox_iou_non_overlapping() {
        let a = BBox::from_corners(0.0, 0.0, 10.0, 10.0);
        let b = BBox::from_corners(20.0, 20.0, 30.0, 30.0);
        assert!((a.iou(&b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn bbox_iou_partial_overlap() {
        let a = BBox::from_corners(0.0, 0.0, 20.0, 20.0);
        let b = BBox::from_corners(10.0, 10.0, 30.0, 30.0);
        // Intersection: 10×10 = 100
        // Union: 400 + 400 - 100 = 700
        // IoU = 100/700 ≈ 0.143
        let iou = a.iou(&b);
        assert!(iou > 0.14 && iou < 0.15, "expected ~0.143, got {iou}");
    }

    #[test]
    fn bbox_area_degenerate() {
        let b = BBox::from_corners(10.0, 10.0, 10.0, 20.0); // zero width
        assert_eq!(b.area(), 0.0);
    }

    #[test]
    fn nms_removes_overlapping_same_class() {
        let dets = vec![
            LayoutRegion {
                class_id: 1,
                class_name: "plain_text".into(),
                confidence: 0.9,
                bbox: BBox::from_corners(0.0, 0.0, 100.0, 100.0),
            },
            LayoutRegion {
                class_id: 1,
                class_name: "plain_text".into(),
                confidence: 0.7,
                bbox: BBox::from_corners(5.0, 5.0, 105.0, 105.0), // high IoU with first
            },
            LayoutRegion {
                class_id: 0,
                class_name: "title".into(),
                confidence: 0.8,
                bbox: BBox::from_corners(0.0, 0.0, 100.0, 100.0), // same box, diff class → kept
            },
        ];
        let result = non_max_suppression(dets, 0.45);
        // Should keep 2: the 0.9 confidence text + the 0.8 title (diff class)
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].confidence, 0.9);
        assert_eq!(result[1].confidence, 0.8);
    }

    #[test]
    fn nms_keeps_non_overlapping() {
        let dets = vec![
            LayoutRegion {
                class_id: 1,
                class_name: "plain_text".into(),
                confidence: 0.9,
                bbox: BBox::from_corners(0.0, 0.0, 50.0, 50.0),
            },
            LayoutRegion {
                class_id: 1,
                class_name: "plain_text".into(),
                confidence: 0.8,
                bbox: BBox::from_corners(200.0, 200.0, 250.0, 250.0), // no overlap
            },
        ];
        let result = non_max_suppression(dets, 0.45);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn nms_empty_input() {
        let result = non_max_suppression(Vec::new(), 0.45);
        assert!(result.is_empty());
    }

    #[test]
    fn decode_yolo_anchor_below_threshold() {
        let anchor = [50.0, 50.0, 20.0, 20.0, 0.1, 0.05, 0.01];
        let result = decode_yolo_anchor(&anchor, 3, 0.25, 1.0, 1.0);
        assert!(result.is_none()); // best score 0.1 < 0.25 threshold
    }

    #[test]
    fn decode_yolo_anchor_valid() {
        // cx=50, cy=50, w=20, h=20, class scores: 0.3, 0.9, 0.1
        let anchor = [50.0, 50.0, 20.0, 20.0, 0.3, 0.9, 0.1];
        let result = decode_yolo_anchor(&anchor, 3, 0.25, 2.0, 2.0).unwrap();
        assert_eq!(result.class_id, 1);
        assert!((result.confidence - 0.9).abs() < 0.001);
        // Box: (50-10)*2=80, (50-10)*2=80, (50+10)*2=120, (50+10)*2=120
        assert!((result.bbox.x - 80.0).abs() < 0.1);
        assert!((result.bbox.right() - 120.0).abs() < 0.1);
    }

    #[test]
    fn post_process_yolo_basic() {
        // 2 anchors, 3 classes, stride=7
        let raw = [
            // Anchor 0: high confidence class 1
            50.0, 50.0, 20.0, 20.0, 0.1, 0.9, 0.05,
            // Anchor 1: below threshold
            10.0, 10.0, 5.0, 5.0, 0.01, 0.02, 0.01,
        ];
        let result = post_process_yolo(&raw, 2, 3, 0.25, 0.45, 1.0, 1.0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].class_id, 1);
    }

    #[test]
    fn doc_layout_classes_count() {
        assert_eq!(DOC_LAYOUT_CLASSES.len(), 12);
        assert_eq!(DOC_LAYOUT_CLASSES[0], "title");
        assert_eq!(DOC_LAYOUT_CLASSES[1], "plain_text");
    }
}

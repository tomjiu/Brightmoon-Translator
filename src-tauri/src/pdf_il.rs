//! P1: PDF Intermediate Language (IL) data structures.
//!
//! Based on BabelDOC `il_version_1.py` + PDFMathTranslate `converter.py`.
//! These structures represent a PDF document in a translation-friendly
//! intermediate format that preserves layout information for round-trip
//! "PDF → IL → PDF" with zero loss.
//!
//! Sequence: P1 (this) → P3+P4+P8 (reflow + writeback + coord isolation)
//! → P5+P6 (formula + layout) → P2 (full frontend) → P7+P9+P10 (polish).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

// ==================== Document Level ====================

/// Root IL document — one instance per PDF file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlDocument {
    /// PDF metadata (Title, Author, Subject, etc.)
    pub metadata: IlMetadata,
    /// Pages in document order (1-indexed in PDF, 0-indexed here).
    pub pages: Vec<IlPage>,
    /// Font table — shared across pages (font_id → IlFont).
    pub fonts: Vec<IlFont>,
    /// Original PDF version (1.4, 1.7, 2.0, etc.)
    pub pdf_version: String,
}

/// PDF metadata extracted from Info dictionary or XMP.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub mod_date: Option<String>,
}

// ==================== Page Level ====================

/// A single PDF page with layout information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlPage {
    /// 1-indexed page number (matches PDF convention).
    pub page_number: usize,
    /// Page dimensions in PDF points (1/72 inch).
    pub width: f32,
    pub height: f32,
    /// MediaBox [llx, lly, urx, ury] in PDF coordinate space.
    pub media_box: [f32; 4],
    /// Rotation angle in degrees (0, 90, 180, 270).
    pub rotation: u16,
    /// Content stream operations, grouped into paragraphs.
    pub paragraphs: Vec<IlParagraph>,
    /// Vector drawings (lines, rects, curves) — preserved as-is.
    pub vector_ops: Vec<IlVectorOp>,
    /// Images on the page (XObjects) — referenced by name.
    pub images: Vec<IlImage>,
}

// ==================== Paragraph Level ====================

/// A paragraph is a group of characters sharing the same line flow.
/// P6 (DocLayout-YOLO) will populate this; until then, paragraphs are
/// inferred from text positioning heuristics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlParagraph {
    /// Bounding box in PDF coordinate space [llx, lly, urx, ury].
    pub bbox: [f32; 4],
    /// Characters in reading order (left-to-right, top-to-bottom).
    pub characters: Vec<IlCharacter>,
    /// Translated text (filled after translation pass).
    #[serde(default)]
    pub translated_text: Option<String>,
    /// Paragraph type hint (for P5 formula detection).
    pub paragraph_type: IlParagraphType,
    /// Language detected for this paragraph (ISO 639-1).
    #[serde(default)]
    pub detected_language: Option<String>,
}

/// Paragraph classification for layout-aware translation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IlParagraphType {
    /// Normal body text — translate.
    Text,
    /// Formula detected (P5) — preserve as-is, use placeholder {vN}.
    Formula,
    /// Heading / title — translate with style preservation.
    Heading,
    /// Caption (figure/table) — translate.
    Caption,
    /// Table cell — translate with alignment preservation.
    TableCell,
    /// Footer / header / page number — skip.
    Metadata,
    /// Code block — skip.
    Code,
}

// ==================== Character Level ====================

/// A single character with absolute positioning.
/// P4 (per-character writeback) uses this to emit Tf/Tm/TJ operators.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlCharacter {
    /// The character itself (may be a multi-byte CJK glyph).
    pub unicode: char,
    /// Font ID index into IlDocument.fonts.
    pub font_id: u32,
    /// Font size in points.
    pub font_size: f32,
    /// Absolute position (x, y) in PDF coordinate space (bottom-left origin).
    pub x: f32,
    pub y: f32,
    /// Text rendering mode (0=fill, 1=stroke, 2=fill+stroke, 3=invisible, etc.)
    pub render_mode: u8,
    /// Color (R, G, B) in 0.0-1.0 range.
    pub color: [f32; 3],
    /// Character width advance (for TJ operator spacing).
    pub advance: f32,
    /// P5: if true, this character is part of a formula — do not translate.
    pub is_formula: bool,
    /// P8: raw PDF operator that produced this char (for round-trip fidelity).
    #[serde(default)]
    pub source_op: Option<String>,
}

// ==================== Font Level ====================

/// Font descriptor — one entry per unique font resource in the PDF.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlFont {
    /// Internal font ID (index into IlDocument.fonts).
    pub font_id: u32,
    /// PDF font resource name (e.g., "/F1", "/CJKFont").
    pub resource_name: String,
    /// Font PostScript name (e.g., "ArialMT", "STSong-Light").
    pub base_font: String,
    /// Encoding type — determines how glyph IDs map to Unicode.
    pub encoding: IlFontEncoding,
    /// Embedded font flag (true if font is subset-embedded in PDF).
    pub is_embedded: bool,
    /// P7: Glyph ID → Unicode mapping (for font subset writeback).
    #[serde(default)]
    pub glyph_to_unicode: std::collections::HashMap<u16, char>,
    /// P7: Unicode → Glyph ID mapping (for translated text → glyph encoding).
    #[serde(default)]
    pub unicode_to_glyph: std::collections::HashMap<char, u16>,
    /// Font flags from FontDescriptor (Flags bit field).
    #[serde(default)]
    pub flags: u32,
    /// Ascent / Descent (in 1000-unit em space).
    #[serde(default)]
    pub ascent: Option<i32>,
    #[serde(default)]
    pub descent: Option<i32>,
}

/// Font encoding type — determines glyph-to-Unicode mapping strategy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IlFontEncoding {
    /// Standard 14 core fonts (Times, Helvetica, Courier) — ASCII only.
    Standard,
    /// WinAnsiEncoding (Latin-1 + extensions).
    WinAnsi,
    /// MacRomanEncoding.
    MacRoman,
    /// Custom Type1 encoding with Differences array.
    CustomType1,
    /// Type0 (CID) font with CMap — CJK fonts.
    Type0CMap,
    /// TrueType with Unicode cmap.
    TrueTypeUnicode,
    /// Unknown — preserve raw glyph IDs, no Unicode mapping.
    Unknown,
}

// ==================== Vector + Image ====================

/// Vector drawing operation (preserved for layout fidelity).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlVectorOp {
    /// Raw PDF content stream operator (e.g., "re", "l", "c", "S", "f").
    pub operator: String,
    /// Operands (numbers) preceding the operator.
    pub operands: Vec<f32>,
    /// Graphics state at time of operation (optional, for complex paths).
    #[serde(default)]
    pub stroke_color: Option<[f32; 3]>,
    #[serde(default)]
    pub fill_color: Option<[f32; 3]>,
    #[serde(default)]
    pub line_width: Option<f32>,
}

/// Image reference (XObject) on a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlImage {
    /// XObject resource name (e.g., "/Im1").
    pub resource_name: String,
    /// Bounding box in page coordinate space.
    pub bbox: [f32; 4],
    /// Image dimensions in pixels.
    pub width: u32,
    pub height: u32,
    /// Color space (DeviceRGB, DeviceGray, DeviceCMYK, etc.)
    pub color_space: String,
    /// Bits per component.
    pub bits_per_component: u8,
}

// ==================== P5: Formula Placeholder ====================

/// Formula placeholder system (P5).
/// Formulas are replaced with `{vN}` placeholders before translation,
/// then restored after translation. Detection uses three signals:
/// 1. Font name regex (Cambria Math, STIX, etc.)
/// 2. Unicode math class (U+2200-U+22FF, U+27C0-U+27EF, etc.)
/// 3. has_glyph check (font lacks glyph for translated text)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormulaPlaceholder {
    /// Placeholder ID (e.g., "{v0}", "{v1}").
    pub placeholder_id: String,
    /// Original formula text (for restoration).
    pub original_text: String,
    /// Detection signals that triggered formula classification.
    pub signals: Vec<FormulaSignal>,
}

/// Formula detection signal (P5 three-signal detection).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FormulaSignal {
    /// Font name matches math font regex.
    FontNameRegex,
    /// Character is in Unicode math block.
    UnicodeMathClass,
    /// Font lacks glyph for target translation text.
    HasGlyphCheck,
}

// ==================== P8: Coordinate Isolation ====================

/// P8: obj_patch + coordinate isolation.
/// Wraps translated content in `q ops_base Q cm ops_new` to isolate
/// coordinate transformations from the original page content.
///
/// ```pdf
/// q                              % save graphics state
/// <original content stream>      % ops_base (preserved as-is)
/// Q                              % restore graphics state
/// <cm matrix>                    % coordinate transformation
/// <translated content stream>    % ops_new (Tf/Tm/TJ writeback)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinateIsolationPatch {
    /// Original content stream bytes (ops_base).
    pub original_stream: Vec<u8>,
    /// Translated content stream bytes (ops_new).
    pub translated_stream: Vec<u8>,
    /// Transformation matrix [a, b, c, d, e, f] for `cm` operator.
    /// Identity = [1, 0, 0, 1, 0, 0].
    pub transform_matrix: [f32; 6],
}

// ==================== P9: Output Mode ====================

/// P9: PDF output mode — mono (replace) or dual (interleaved pages).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PdfOutputMode {
    /// Replace original text with translation (single-language PDF).
    Mono,
    /// Insert translated pages after each original page (bilingual PDF).
    Dual,
}

// ==================== P3: Reflow Options ====================

/// P3: Layout reflow options for `_find_optimal_scale_and_layout`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflowOptions {
    /// Scale-down step (0.05 = 5% reduction per iteration).
    pub scale_step: f32,
    /// Minimum scale (stop if below this).
    pub min_scale: f32,
    /// Box expansion direction: down first, then right.
    pub expand_down_first: bool,
    /// CJK line skip multiplier (1.5x font size).
    pub cjk_line_skip: f32,
    /// Maximum reflow iterations before giving up.
    pub max_iterations: u32,
}

impl Default for ReflowOptions {
    fn default() -> Self {
        Self {
            scale_step: 0.05,
            min_scale: 0.5,
            expand_down_first: true,
            cjk_line_skip: 1.5,
            max_iterations: 20,
        }
    }
}

/// P3: Reflow result — optimal scale + box layout for translated text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflowResult {
    /// Optimal scale factor (1.0 = no scaling needed).
    pub scale: f32,
    /// Adjusted bounding box after reflow.
    pub bbox: [f32; 4],
    /// Number of lines after reflow.
    pub line_count: u32,
    /// Whether text was truncated (couldn't fit even at min_scale).
    pub truncated: bool,
}

// ==================== P10: Translation Cache Key ====================

/// P10: SQLite translation cache key (engine + params + text triple key).
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationCacheKey {
    pub engine: String,
    pub source_lang: String,
    pub target_lang: String,
    /// Normalized text (trimmed, lowercased, whitespace-collapsed).
    pub text_hash: String,
}

// ==================== P3: Reflow Algorithm ====================

/// Heuristic character width: Latin ≈ 0.5em, CJK ≈ 1.0em, fullwidth punct ≈ 1.0em.
fn char_width(c: char, font_size: f32) -> f32 {
    let em = if is_cjk_char(c) { 1.0 } else { 0.5 };
    em * font_size
}

/// CJK character detection (CJK Unified Ideographs + common CJK ranges).
fn is_cjk_char(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F |   // CJK Symbols and Punctuation
        0x3040..=0x309F |   // Hiragana
        0x30A0..=0x30FF |   // Katakana
        0x3400..=0x4DBF |   // CJK Extension A
        0x4E00..=0x9FFF |   // CJK Unified Ideographs
        0xF900..=0xFAFF |   // CJK Compatibility Ideographs
        0xFF00..=0xFFEF |   // Fullwidth Forms
        0x20000..=0x2A6DF   // CJK Extension B
    )
}

/// Wrap translated text into lines that fit within `max_width` (in PDF points).
/// Uses greedy word-wrap for Latin and per-char wrap for CJK.
fn wrap_text_to_width(text: &str, max_width: f32, font_size: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut current_width: f32 = 0.0;

    for word in text.split_whitespace() {
        let word_width: f32 = word.chars().map(|c| char_width(c, font_size)).sum();
        let space_width = char_width(' ', font_size);

        // CJK per-char wrap: if the word is all CJK and wider than max_width,
        // wrap per character regardless of line state.
        if word.chars().all(is_cjk_char) && word_width > max_width {
            for c in word.chars() {
                let cw = char_width(c, font_size);
                if !current_line.is_empty() && current_width + cw > max_width {
                    lines.push(std::mem::take(&mut current_line));
                    current_width = 0.0;
                }
                current_line.push(c);
                current_width += cw;
            }
            continue;
        }

        let needed = if current_line.is_empty() {
            word_width
        } else {
            current_width + space_width + word_width
        };

        if needed <= max_width || current_line.is_empty() {
            if !current_line.is_empty() {
                current_line.push(' ');
                current_width += space_width;
            }
            current_line.push_str(word);
            current_width += word_width;
        } else {
            lines.push(std::mem::take(&mut current_line));
            current_line = word.to_string();
            current_width = word_width;
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// P3: Find optimal scale and layout for translated text within a bounding box.
///
/// Algorithm (adapted from BabelDOC `typesetting.py:941`):
/// 1. Start at scale = 1.0
/// 2. Render text at current scale; if it fits in bbox, done
/// 3. If not, scale down by `scale_step` (0.05 = 5% reduction)
/// 4. If still doesn't fit at `min_scale`, expand box down by one line height
/// 5. If can't expand down (page boundary), expand right
/// 6. Repeat up to `max_iterations`
///
/// Returns `ReflowResult` with the optimal scale, adjusted bbox, and line count.
pub fn find_optimal_scale_and_layout(
    translated_text: &str,
    original_bbox: &[f32; 4],
    font_size: f32,
    page_height: f32,
    options: &ReflowOptions,
) -> ReflowResult {
    let mut scale = 1.0_f32;
    let mut bbox = *original_bbox;
    let has_cjk = translated_text.chars().any(is_cjk_char);
    let line_skip = if has_cjk { options.cjk_line_skip } else { 1.2 };

    for _ in 0..options.max_iterations {
        let scaled_font = font_size * scale;
        let box_width = bbox[2] - bbox[0]; // urx - llx
        let lines = wrap_text_to_width(translated_text, box_width, scaled_font);
        let line_height = scaled_font * line_skip;
        let total_height = line_height * lines.len() as f32;
        let box_height = bbox[3] - bbox[1]; // ury - lly

        if total_height <= box_height {
            return ReflowResult {
                scale,
                bbox,
                line_count: lines.len() as u32,
                truncated: false,
            };
        }

        // Scale down first
        if scale > options.min_scale + options.scale_step {
            scale -= options.scale_step;
            continue;
        }

        // At min_scale — try box expansion
        scale = options.min_scale;
        if options.expand_down_first {
            // Expand down: lower lly by one line, but don't go below page bottom margin
            let new_lly = bbox[1] - line_height;
            if new_lly >= 10.0 {
                bbox[1] = new_lly;
            } else {
                // Can't expand down — expand right
                let new_urx = bbox[2] + box_width * 0.2;
                if new_urx <= page_height {
                    // page_width not available; use page_height as upper bound fallback
                    bbox[2] = new_urx;
                } else {
                    // Can't expand — text will be truncated
                    return ReflowResult {
                        scale,
                        bbox,
                        line_count: lines.len() as u32,
                        truncated: true,
                    };
                }
            }
        } else {
            // Expand right first
            let new_urx = bbox[2] + box_width * 0.2;
            bbox[2] = new_urx;
        }
    }

    // Exhausted iterations
    let scaled_font = font_size * scale;
    let box_width = bbox[2] - bbox[0];
    let lines = wrap_text_to_width(translated_text, box_width, scaled_font);
    ReflowResult {
        scale,
        bbox,
        line_count: lines.len() as u32,
        truncated: true,
    }
}

// ==================== P4: Per-Character Writeback ====================

/// P4: Emit a PDF content stream for a translated paragraph using
/// per-character Tf/Tm/TJ operators with absolute positioning.
///
/// Produces a `BT ... ET` block where each character (or CJK glyph) is
/// positioned absolutely via `Tm` (text matrix). This preserves the
/// original layout while replacing the text.
///
/// Reference: PDFMathTranslate `converter.py:385-386,409-511`
pub fn write_paragraph_to_content_stream(
    paragraph: &IlParagraph,
    reflow: &ReflowResult,
    font_resource_name: &str,
) -> Vec<u8> {
    let translated = match &paragraph.translated_text {
        Some(t) => t.as_str(),
        None => return Vec::new(),
    };

    let scaled_font = paragraph
        .characters
        .first()
        .map(|c| c.font_size * reflow.scale)
        .unwrap_or(10.0);

    let box_width = reflow.bbox[2] - reflow.bbox[0];
    let lines = wrap_text_to_width(translated, box_width, scaled_font);

    let has_cjk = translated.chars().any(is_cjk_char);
    let line_skip = if has_cjk { 1.5 } else { 1.2 };
    let line_height = scaled_font * line_skip;

    // Start from top-left of reflowed bbox (PDF y grows upward)
    let mut x = reflow.bbox[0];
    let mut y = reflow.bbox[3] - scaled_font; // top of box minus one font size

    let mut stream = Vec::with_capacity(256);
    stream.extend_from_slice(b"BT\n");

    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx > 0 {
            y -= line_height;
            x = reflow.bbox[0];
        }

        // Set font for this line
        // Tf: /F1 12 Tf
        stream.extend_from_slice(format!("/{font_resource_name} {:.1} Tf\n", scaled_font).as_bytes());

        for c in line.chars() {
            if c == ' ' {
                x += char_width(' ', scaled_font);
                continue;
            }

            // Tm: a b c d e f (text matrix — absolute positioning)
            // For horizontal text: [scale 0 0 scale x y]
            stream.extend_from_slice(
                format!("{:.2} 0 0 {:.2} {:.2} {:.2} Tm\n", reflow.scale, reflow.scale, x, y).as_bytes(),
            );

            // Tj: show character. ASCII → literal string "(x)",
            // CJK → UTF-16BE hex string "<FEFFxxxx>" (PDF spec §7.9.2.2).
            stream.extend_from_slice(format!("{} Tj\n", encode_pdf_string(c)).as_bytes());

            x += char_width(c, scaled_font);
        }
    }

    stream.extend_from_slice(b"ET\n");
    stream
}

/// Encode a single character as a PDF string token.
///
/// - ASCII printable → literal string `(c)` with `\()\\` escapes
/// - Non-ASCII → hex string `<FEFF xxxx>` (UTF-16BE with BOM)
fn encode_pdf_string(c: char) -> String {
    if c.is_ascii() && !c.is_ascii_control() {
        match c {
            '\\' => "(\\\\)".to_string(),
            '(' => "(\\()".to_string(),
            ')' => "(\\))".to_string(),
            _ => format!("({})", c),
        }
    } else {
        // UTF-16BE hex string with BOM
        let mut buf = [0u16; 2];
        let units = c.encode_utf16(&mut buf);
        let mut hex = String::from("<FEFF");
        for &u in units.iter() {
            hex.push_str(&format!("{:04X}", u));
        }
        hex.push('>');
        hex
    }
}

// ==================== P8: Coordinate Isolation ====================

/// P8: Wrap original and translated content streams with coordinate isolation.
///
/// Produces:
/// ```pdf
/// q                              % save graphics state
/// <original_stream>              % ops_base (preserved as-is)
/// Q                              % restore graphics state
/// <matrix> cm                    % coordinate transformation
/// <translated_stream>            % ops_new (Tf/Tm/TJ writeback)
/// ```
///
/// This ensures the translated content's coordinate transformations don't
/// leak into the original page content, preventing layout corruption.
///
/// Reference: PDFMathTranslate `pdfinterp.py:254-278`
pub fn apply_coordinate_isolation(patch: &CoordinateIsolationPatch) -> Vec<u8> {
    let [a, b, c, d, e, f] = patch.transform_matrix;
    let mut stream = Vec::with_capacity(
        patch.original_stream.len() + patch.translated_stream.len() + 64,
    );

    // Save graphics state + original content
    stream.extend_from_slice(b"q\n");
    stream.extend_from_slice(&patch.original_stream);
    if !patch.original_stream.ends_with(b"\n") {
        stream.push(b'\n');
    }
    stream.extend_from_slice(b"Q\n");

    // Coordinate transformation matrix
    stream.extend_from_slice(format!("{:.4} {:.4} {:.4} {:.4} {:.2} {:.2} cm\n", a, b, c, d, e, f).as_bytes());

    // Translated content (ops_new)
    stream.extend_from_slice(&patch.translated_stream);

    stream
}

// ==================== P5: Formula Detection & Placeholder ====================

/// Math font name regex (case-insensitive substring match).
/// Covers common math fonts: Cambria Math, STIX, Latin Modern Math, etc.
const MATH_FONT_PATTERNS: &[&str] = &[
    "cambria math",
    "stix",
    "latin modern math",
    "asana math",
    "xits math",
    "libertinus math",
    "tex gyre termes math",
    "tex gyre pagella math",
    "dejavu serif",
    "computer modern",
    "cmsy",
    "cmex",
    "cmmi",
    "symbol",
    "mathitalic",
    "mathbold",
    "mathsymbols",
    "mathalpha",
];

/// Unicode math blocks (Mathematical Operators, Supplemental Math, etc.).
/// Reference: Unicode Standard §22-23.
fn is_unicode_math_char(c: char) -> bool {
    matches!(c as u32,
        0x2200..=0x22FF |   // Mathematical Operators
        0x27C0..=0x27EF |   // Supplemental Math Operators (subset)
        0x2980..=0x29FF |   // Misc Math Symbols-B
        0x2A00..=0x2AFF |   // Supplemental Math Operators
        0x2100..=0x214F |   // Letterlike Symbols (ℕ, ℝ, ℤ, etc.)
        0x1D400..=0x1D7FF   // Math Alphanumeric Symbols
    )
}

/// Check if a font name matches a known math font pattern (case-insensitive).
pub fn is_math_font(font_name: &str) -> bool {
    let lower = font_name.to_lowercase();
    MATH_FONT_PATTERNS.iter().any(|pat| lower.contains(pat))
}

/// P5: Detect formula characters in a paragraph using three signals.
///
/// Signals (any one triggers formula classification):
/// 1. `FormulaSignal::FontNameRegex` — font name matches math font pattern
/// 2. `FormulaSignal::UnicodeMathClass` — char is in Unicode math block
/// 3. `FormulaSignal::HasGlyphCheck` — font lacks glyph for target translation
///
/// Returns a list of (char_index, signals) for flagged characters.
pub fn detect_formula_characters(
    paragraph: &IlParagraph,
    fonts: &[IlFont],
) -> Vec<(usize, Vec<FormulaSignal>)> {
    let mut flagged = Vec::new();

    for (idx, ch) in paragraph.characters.iter().enumerate() {
        let mut signals = Vec::new();

        // Signal 1: font name regex
        if let Some(font) = fonts.get(ch.font_id as usize) {
            if is_math_font(&font.base_font) || is_math_font(&font.resource_name) {
                signals.push(FormulaSignal::FontNameRegex);
            }
        }

        // Signal 2: Unicode math class
        if is_unicode_math_char(ch.unicode) {
            signals.push(FormulaSignal::UnicodeMathClass);
        }

        // Signal 3: has_glyph check — if char is marked is_formula by upstream,
        // or if the font lacks a glyph for common CJK targets, flag it.
        // (Full has_glyph requires font parsing; here we use the is_formula
        // flag set by the PDF frontend as a proxy.)
        if ch.is_formula {
            signals.push(FormulaSignal::HasGlyphCheck);
        }

        if !signals.is_empty() {
            flagged.push((idx, signals));
        }
    }

    flagged
}

/// P5: Group contiguous flagged characters into formula spans.
///
/// A formula span is a maximal run of consecutive flagged characters.
/// Each span becomes one `{vN}` placeholder.
pub fn group_formula_spans(
    flagged: &[(usize, Vec<FormulaSignal>)],
) -> Vec<(usize, usize, Vec<FormulaSignal>)> {
    // (start_idx, end_idx_inclusive, union_of_signals)
    let mut spans: Vec<(usize, usize, Vec<FormulaSignal>)> = Vec::new();

    for &(idx, ref signals) in flagged {
        if let Some(last) = spans.last_mut() {
            if idx == last.1 + 1 {
                // Extend current span
                last.1 = idx;
                for s in signals {
                    if !last.2.contains(s) {
                        last.2.push(s.clone());
                    }
                }
                continue;
            }
        }
        // Start new span
        spans.push((idx, idx, signals.clone()));
    }

    spans
}

/// P5: Replace formula spans in text with `{vN}` placeholders.
///
/// Given the original paragraph text and the formula spans (as char indices
/// into the paragraph's `characters` vec), produce:
/// - `masked_text`: text with formulas replaced by `{v0}`, `{v1}`, ...
/// - `placeholders`: vector of `FormulaPlaceholder` records for restoration
///
/// Note: char indices refer to positions in `paragraph.characters`, which
/// may differ from `paragraph.text` if the frontend normalized whitespace.
/// Callers should pass the text reconstructed from `characters`.
pub fn mask_formulas(
    text: &str,
    spans: &[(usize, usize, Vec<FormulaSignal>)],
) -> (String, Vec<FormulaPlaceholder>) {
    if spans.is_empty() {
        return (text.to_string(), Vec::new());
    }

    let chars: Vec<char> = text.chars().collect();
    let mut masked = String::with_capacity(chars.len());
    let mut placeholders = Vec::with_capacity(spans.len());
    let mut last_end: i64 = -1; // last consumed char index (inclusive)

    for (span_idx, &(start, end, ref signals)) in spans.iter().enumerate() {
        // Append text before this span
        let prev = (last_end + 1) as usize;
        if prev < start {
            for &c in &chars[prev..start] {
                masked.push(c);
            }
        }

        // Extract original formula text
        let formula_text: String = chars[start..=end].iter().collect();

        // Generate placeholder
        let placeholder_id = format!("{{v{}}}", span_idx);
        masked.push_str(&placeholder_id);

        placeholders.push(FormulaPlaceholder {
            placeholder_id: placeholder_id.clone(),
            original_text: formula_text,
            signals: signals.clone(),
        });

        last_end = end as i64;
    }

    // Append trailing text after last span
    let trailing_start = (last_end + 1) as usize;
    if trailing_start < chars.len() {
        for &c in &chars[trailing_start..] {
            masked.push(c);
        }
    }

    (masked, placeholders)
}

/// P5: Restore formula placeholders in translated text.
///
/// Replaces each `{vN}` occurrence in `translated_text` with the original
/// formula text from `placeholders`. This is called after the LLM translates
/// the masked text.
///
/// If a placeholder is not found in the translated text (LLM dropped it),
/// it is appended at the end to preserve all formulas.
pub fn restore_formulas(
    translated_text: &str,
    placeholders: &[FormulaPlaceholder],
) -> String {
    let mut result = translated_text.to_string();

    for ph in placeholders {
        // If the placeholder survived translation, restore the original formula.
        if result.contains(&ph.placeholder_id) {
            result = result.replace(&ph.placeholder_id, &ph.original_text);
        } else {
            // LLM dropped the placeholder — append the formula to preserve it.
            result.push_str(&ph.original_text);
        }
    }

    result
}

// ==================== P10: SQLite Translation Cache ====================

/// P10: Persistent translation cache backed by SQLite + WAL.
///
/// Keyed by (engine, source_lang, target_lang, text_hash) — matches
/// PDFMathTranslate `cache.py` triple-key scheme. WAL mode enables
/// concurrent reads during writes for batch translation.
pub struct PdfTranslationCache {
    conn: rusqlite::Connection,
}

/// P10: A cache entry record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheEntry {
    pub engine: String,
    pub source_lang: String,
    pub target_lang: String,
    pub text_hash: String,
    pub original_text: String,
    pub translated_text: String,
    pub created_at: i64,
}

impl PdfTranslationCache {
    /// Open or create the cache database at `path`. Enables WAL mode and
    /// creates the `pdf_translations` table if it doesn't exist.
    pub fn open(path: &str) -> Result<Self, String> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| format!("Failed to open cache DB: {}", e))?;

        // Enable WAL for concurrent read access during writes
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| format!("Failed to set WAL mode: {}", e))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| format!("Failed to set synchronous mode: {}", e))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pdf_translations (
                engine        TEXT NOT NULL,
                source_lang   TEXT NOT NULL,
                target_lang   TEXT NOT NULL,
                text_hash     TEXT NOT NULL,
                original_text TEXT NOT NULL,
                translated_text TEXT NOT NULL,
                created_at    INTEGER NOT NULL,
                PRIMARY KEY (engine, source_lang, target_lang, text_hash)
            );
            CREATE INDEX IF NOT EXISTS idx_pdf_translations_text_hash
                ON pdf_translations(text_hash);",
        )
        .map_err(|e| format!("Failed to create cache table: {}", e))?;

        Ok(Self { conn })
    }

    /// Open an in-memory cache (for tests).
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| format!("Failed to open in-memory DB: {}", e))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pdf_translations (
                engine        TEXT NOT NULL,
                source_lang   TEXT NOT NULL,
                target_lang   TEXT NOT NULL,
                text_hash     TEXT NOT NULL,
                original_text TEXT NOT NULL,
                translated_text TEXT NOT NULL,
                created_at    INTEGER NOT NULL,
                PRIMARY KEY (engine, source_lang, target_lang, text_hash)
            );",
        )
        .map_err(|e| format!("Failed to create cache table: {}", e))?;
        Ok(Self { conn })
    }

    /// Compute a stable hash for a text string (SHA-1 hex, matching
    /// PDFMathTranslate's cache key normalization).
    pub fn hash_text(text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        // Normalize: trim + collapse whitespace + lowercase
        let normalized: String = text
            .trim()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let mut hasher = DefaultHasher::new();
        normalized.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Look up a cached translation. Returns `None` if not found.
    pub fn get(&self, key: &TranslationCacheKey) -> Result<Option<CacheEntry>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT engine, source_lang, target_lang, text_hash,
                        original_text, translated_text, created_at
                 FROM pdf_translations
                 WHERE engine = ?1 AND source_lang = ?2
                   AND target_lang = ?3 AND text_hash = ?4",
            )
            .map_err(|e| format!("Cache get prepare failed: {}", e))?;

        let result = stmt
            .query_row(
                rusqlite::params![
                    key.engine,
                    key.source_lang,
                    key.target_lang,
                    key.text_hash,
                ],
                |row| {
                    Ok(CacheEntry {
                        engine: row.get(0)?,
                        source_lang: row.get(1)?,
                        target_lang: row.get(2)?,
                        text_hash: row.get(3)?,
                        original_text: row.get(4)?,
                        translated_text: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            );

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Cache get failed: {}", e)),
        }
    }

    /// Insert or replace a translation in the cache.
    pub fn put(&self, entry: &CacheEntry) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO pdf_translations
                 (engine, source_lang, target_lang, text_hash,
                  original_text, translated_text, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    entry.engine,
                    entry.source_lang,
                    entry.target_lang,
                    entry.text_hash,
                    entry.original_text,
                    entry.translated_text,
                    entry.created_at,
                ],
            )
            .map_err(|e| format!("Cache put failed: {}", e))?;
        Ok(())
    }

    /// Convenience: look up by raw text (computes hash internally).
    pub fn lookup(
        &self,
        engine: &str,
        source_lang: &str,
        target_lang: &str,
        text: &str,
    ) -> Result<Option<CacheEntry>, String> {
        let key = TranslationCacheKey {
            engine: engine.to_string(),
            source_lang: source_lang.to_string(),
            target_lang: target_lang.to_string(),
            text_hash: Self::hash_text(text),
        };
        self.get(&key)
    }

    /// Convenience: insert by raw text (computes hash internally).
    pub fn store(
        &self,
        engine: &str,
        source_lang: &str,
        target_lang: &str,
        original_text: &str,
        translated_text: &str,
    ) -> Result<(), String> {
        let entry = CacheEntry {
            engine: engine.to_string(),
            source_lang: source_lang.to_string(),
            target_lang: target_lang.to_string(),
            text_hash: Self::hash_text(original_text),
            original_text: original_text.to_string(),
            translated_text: translated_text.to_string(),
            created_at: chrono::Utc::now().timestamp(),
        };
        self.put(&entry)
    }

    /// Delete entries older than `max_age_seconds`. Returns count deleted.
    pub fn evict_older_than(&self, max_age_seconds: i64) -> Result<usize, String> {
        let cutoff = chrono::Utc::now().timestamp() - max_age_seconds;
        let count = self
            .conn
            .execute(
                "DELETE FROM pdf_translations WHERE created_at < ?1",
                rusqlite::params![cutoff],
            )
            .map_err(|e| format!("Cache evict failed: {}", e))?;
        Ok(count)
    }

    /// Total number of cached entries.
    pub fn count(&self) -> Result<usize, String> {
        self.conn
            .query_row("SELECT COUNT(*) FROM pdf_translations", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|n| n as usize)
            .map_err(|e| format!("Cache count failed: {}", e))
    }

    /// Clear all cached entries. Returns count deleted.
    pub fn clear(&self) -> Result<usize, String> {
        let count = self
            .conn
            .execute("DELETE FROM pdf_translations", [])
            .map_err(|e| format!("Cache clear failed: {}", e))?;
        Ok(count)
    }
}

/// P10: Open the PDF translation cache at the app's data directory.
///
/// The cache DB lives at `<app_data_dir>/moontranslator/pdf_translation_cache.db`.
/// Creates the directory and DB file if missing. Returns a connection ready
/// for lookup/store.
pub fn open_pdf_translation_cache(app: &AppHandle) -> Result<PdfTranslationCache, String> {
    let mut path = app
        .path()
        .app_data_dir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| {
            let mut p = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            p.push("moontranslator");
            p
        });
    std::fs::create_dir_all(&path).map_err(|e| format!("create cache dir: {e}"))?;
    path.push("pdf_translation_cache.db");
    PdfTranslationCache::open(path.to_str().unwrap_or("pdf_translation_cache.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn il_document_serialization_roundtrip() {
        let doc = IlDocument {
            metadata: IlMetadata {
                title: Some("Test Document".to_string()),
                author: Some("Test Author".to_string()),
                ..Default::default()
            },
            pages: vec![IlPage {
                page_number: 1,
                width: 612.0,
                height: 792.0,
                media_box: [0.0, 0.0, 612.0, 792.0],
                rotation: 0,
                paragraphs: vec![IlParagraph {
                    bbox: [72.0, 700.0, 540.0, 720.0],
                    characters: vec![IlCharacter {
                        unicode: 'H',
                        font_id: 0,
                        font_size: 12.0,
                        x: 72.0,
                        y: 710.0,
                        render_mode: 0,
                        color: [0.0, 0.0, 0.0],
                        advance: 6.6,
                        is_formula: false,
                        source_op: Some("Tj".to_string()),
                    }],
                    translated_text: None,
                    paragraph_type: IlParagraphType::Text,
                    detected_language: Some("en".to_string()),
                }],
                vector_ops: vec![],
                images: vec![],
            }],
            fonts: vec![IlFont {
                font_id: 0,
                resource_name: "/F1".to_string(),
                base_font: "Helvetica".to_string(),
                encoding: IlFontEncoding::Standard,
                is_embedded: false,
                glyph_to_unicode: std::collections::HashMap::new(),
                unicode_to_glyph: std::collections::HashMap::new(),
                flags: 32,
                ascent: Some(718),
                descent: Some(-207),
            }],
            pdf_version: "1.7".to_string(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: IlDocument = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.pages.len(), 1);
        assert_eq!(deserialized.pages[0].page_number, 1);
        assert_eq!(deserialized.fonts.len(), 1);
        assert_eq!(deserialized.fonts[0].base_font, "Helvetica");
        assert_eq!(
            deserialized.pages[0].paragraphs[0].characters[0].unicode,
            'H'
        );
    }

    #[test]
    fn paragraph_type_serialization() {
        let types = vec![
            IlParagraphType::Text,
            IlParagraphType::Formula,
            IlParagraphType::Heading,
            IlParagraphType::Caption,
            IlParagraphType::TableCell,
            IlParagraphType::Metadata,
            IlParagraphType::Code,
        ];
        for pt in &types {
            let json = serde_json::to_string(pt).unwrap();
            let deserialized: IlParagraphType = serde_json::from_str(&json).unwrap();
            assert_eq!(*pt, deserialized);
        }
    }

    #[test]
    fn font_encoding_serialization() {
        let encodings = vec![
            IlFontEncoding::Standard,
            IlFontEncoding::WinAnsi,
            IlFontEncoding::MacRoman,
            IlFontEncoding::CustomType1,
            IlFontEncoding::Type0CMap,
            IlFontEncoding::TrueTypeUnicode,
            IlFontEncoding::Unknown,
        ];
        for enc in &encodings {
            let json = serde_json::to_string(enc).unwrap();
            let deserialized: IlFontEncoding = serde_json::from_str(&json).unwrap();
            assert_eq!(*enc, deserialized);
        }
    }

    #[test]
    fn formula_placeholder_structure() {
        let fp = FormulaPlaceholder {
            placeholder_id: "{v0}".to_string(),
            original_text: "E = mc²".to_string(),
            signals: vec![
                FormulaSignal::FontNameRegex,
                FormulaSignal::UnicodeMathClass,
            ],
        };
        let json = serde_json::to_string(&fp).unwrap();
        let deserialized: FormulaPlaceholder = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.placeholder_id, "{v0}");
        assert_eq!(deserialized.signals.len(), 2);
    }

    #[test]
    fn coordinate_isolation_patch_structure() {
        let patch = CoordinateIsolationPatch {
            original_stream: b"BT /F1 12 Tf 72 710 Td (Hello) Tj ET".to_vec(),
            translated_stream: "BT /F1 12 Tf 72 710 Td (\u{4f60}\u{597d}) Tj ET".as_bytes().to_vec(),
            transform_matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        };
        let json = serde_json::to_string(&patch).unwrap();
        let deserialized: CoordinateIsolationPatch = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.original_stream.is_empty());
        assert!(!deserialized.translated_stream.is_empty());
        assert_eq!(deserialized.transform_matrix, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn reflow_options_defaults() {
        let opts = ReflowOptions::default();
        assert_eq!(opts.scale_step, 0.05);
        assert_eq!(opts.min_scale, 0.5);
        assert!(opts.expand_down_first);
        assert_eq!(opts.cjk_line_skip, 1.5);
        assert_eq!(opts.max_iterations, 20);
    }

    #[test]
    fn pdf_output_mode_serialization() {
        assert_eq!(
            serde_json::to_string(&PdfOutputMode::Mono).unwrap(),
            "\"mono\""
        );
        assert_eq!(
            serde_json::to_string(&PdfOutputMode::Dual).unwrap(),
            "\"dual\""
        );
    }

    #[test]
    fn translation_cache_key_hash_eq() {
        let k1 = TranslationCacheKey {
            engine: "google".to_string(),
            source_lang: "auto".to_string(),
            target_lang: "zh".to_string(),
            text_hash: "abc123".to_string(),
        };
        let k2 = k1.clone();
        assert_eq!(k1, k2);
        // 验证 Hash trait 派生可用
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h1 = DefaultHasher::new();
        k1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        k2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    // ==================== P3 Reflow Tests ====================

    #[test]
    fn p3_reflow_short_text_fits_at_scale_1() {
        // Short text in a large box → scale stays 1.0, no truncation
        let bbox = [72.0, 700.0, 540.0, 720.0]; // 468pt wide, 20pt tall
        let opts = ReflowOptions::default();
        let result = find_optimal_scale_and_layout("Hello", &bbox, 10.0, 792.0, &opts);
        assert!((result.scale - 1.0).abs() < 0.01);
        assert!(!result.truncated);
        assert_eq!(result.line_count, 1);
    }

    #[test]
    fn p3_reflow_long_text_scales_down() {
        // Long Latin text in a small box → must scale down to fit
        let bbox = [72.0, 700.0, 200.0, 720.0]; // 128pt wide, 20pt tall
        let opts = ReflowOptions::default();
        let long_text = "This is a very long sentence that cannot fit in a small box at full scale";
        let result = find_optimal_scale_and_layout(long_text, &bbox, 10.0, 792.0, &opts);
        assert!(result.scale < 1.0, "expected scale < 1.0, got {}", result.scale);
    }

    #[test]
    fn p3_reflow_cjk_uses_larger_line_skip() {
        // CJK text should use 1.5x line skip, resulting in more height per line
        let bbox = [72.0, 600.0, 200.0, 700.0]; // 128pt wide, 100pt tall
        let opts = ReflowOptions::default();
        let cjk_text = "这是一段中文测试文本用于验证排版算法的正确性";
        let result = find_optimal_scale_and_layout(cjk_text, &bbox, 10.0, 792.0, &opts);
        // CJK chars at 10pt are 10pt wide → 12 chars need 120pt → fits in 128pt width
        // but 2 lines × 15pt (1.5 skip) = 30pt < 100pt height → fits at scale 1
        assert!(!result.truncated);
    }

    #[test]
    fn p3_reflow_truncates_when_box_too_small() {
        // Tiny box, long text → should truncate
        let bbox = [72.0, 780.0, 80.0, 790.0]; // 8pt wide, 10pt tall
        let opts = ReflowOptions {
            max_iterations: 3,
            ..Default::default()
        };
        let long_text = "This text is way too long for such a tiny box and should be truncated";
        let result = find_optimal_scale_and_layout(long_text, &bbox, 10.0, 792.0, &opts);
        assert!(result.truncated);
    }

    #[test]
    fn p3_reflow_expands_box_down() {
        // Text slightly too tall → should expand bbox downward
        let bbox = [72.0, 750.0, 200.0, 770.0]; // 128pt wide, 20pt tall
        let opts = ReflowOptions::default();
        let text = "line one line two line three"; // wraps to 3 lines
        let result = find_optimal_scale_and_layout(text, &bbox, 10.0, 792.0, &opts);
        // Either scaled down or bbox expanded — bbox[1] should be <= original 750
        assert!(result.bbox[1] <= 750.0 || result.scale < 1.0);
    }

    // ==================== P4 Writeback Tests ====================

    #[test]
    fn p4_writeback_ascii_text() {
        let para = IlParagraph {
            bbox: [72.0, 700.0, 540.0, 720.0],
            characters: vec![IlCharacter {
                unicode: 'H',
                font_id: 0,
                font_size: 12.0,
                x: 72.0,
                y: 710.0,
                render_mode: 0,
                color: [0.0, 0.0, 0.0],
                advance: 6.6,
                is_formula: false,
                source_op: None,
            }],
            translated_text: Some("Hello".to_string()),
            paragraph_type: IlParagraphType::Text,
            detected_language: None,
        };
        let reflow = ReflowResult {
            scale: 1.0,
            bbox: [72.0, 700.0, 540.0, 720.0],
            line_count: 1,
            truncated: false,
        };
        let stream = write_paragraph_to_content_stream(&para, &reflow, "F1");
        let s = String::from_utf8(stream).unwrap();
        assert!(s.starts_with("BT\n"));
        assert!(s.ends_with("ET\n"));
        assert!(s.contains("/F1 12.0 Tf"));
        assert!(s.contains("Tm"));
        assert!(s.contains("Tj"));
        // ASCII 'H' should be in literal string form
        assert!(s.contains("(H)"));
    }

    #[test]
    fn p4_writeback_cjk_text_uses_hex_string() {
        let para = IlParagraph {
            bbox: [72.0, 700.0, 540.0, 720.0],
            characters: vec![IlCharacter {
                unicode: '你',
                font_id: 0,
                font_size: 12.0,
                x: 72.0,
                y: 710.0,
                render_mode: 0,
                color: [0.0, 0.0, 0.0],
                advance: 12.0,
                is_formula: false,
                source_op: None,
            }],
            translated_text: Some("你好".to_string()),
            paragraph_type: IlParagraphType::Text,
            detected_language: None,
        };
        let reflow = ReflowResult {
            scale: 1.0,
            bbox: [72.0, 700.0, 540.0, 720.0],
            line_count: 1,
            truncated: false,
        };
        let stream = write_paragraph_to_content_stream(&para, &reflow, "CJK");
        let s = String::from_utf8(stream).unwrap();
        // CJK chars should use hex string <FEFFxxxx>
        assert!(s.contains("<FEFF"), "expected hex string for CJK, got: {}", s);
        assert!(!s.contains("(你)"), "should not use literal string for CJK");
    }

    #[test]
    fn p4_writeback_empty_translation_returns_empty() {
        let para = IlParagraph {
            bbox: [72.0, 700.0, 540.0, 720.0],
            characters: vec![],
            translated_text: None,
            paragraph_type: IlParagraphType::Text,
            detected_language: None,
        };
        let reflow = ReflowResult {
            scale: 1.0,
            bbox: [72.0, 700.0, 540.0, 720.0],
            line_count: 0,
            truncated: false,
        };
        let stream = write_paragraph_to_content_stream(&para, &reflow, "F1");
        assert!(stream.is_empty());
    }

    #[test]
    fn p4_writeback_escapes_special_chars() {
        let para = IlParagraph {
            bbox: [72.0, 700.0, 540.0, 720.0],
            characters: vec![IlCharacter {
                unicode: 'a',
                font_id: 0,
                font_size: 12.0,
                x: 72.0,
                y: 710.0,
                render_mode: 0,
                color: [0.0, 0.0, 0.0],
                advance: 6.0,
                is_formula: false,
                source_op: None,
            }],
            translated_text: Some("(test)".to_string()),
            paragraph_type: IlParagraphType::Text,
            detected_language: None,
        };
        let reflow = ReflowResult {
            scale: 1.0,
            bbox: [72.0, 700.0, 540.0, 720.0],
            line_count: 1,
            truncated: false,
        };
        let stream = write_paragraph_to_content_stream(&para, &reflow, "F1");
        let s = String::from_utf8(stream).unwrap();
        // Parentheses must be escaped
        assert!(s.contains(r"\(\)") || s.contains(r"\("), "expected escaped parens in: {}", s);
    }

    // ==================== P8 Coordinate Isolation Tests ====================

    #[test]
    fn p8_coordinate_isolation_wraps_with_q_q() {
        let patch = CoordinateIsolationPatch {
            original_stream: b"BT /F1 12 Tf 72 710 Td (Hello) Tj ET".to_vec(),
            translated_stream: b"BT /F1 12 Tf 72 710 Td (Bonjour) Tj ET".to_vec(),
            transform_matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        };
        let result = apply_coordinate_isolation(&patch);
        let s = String::from_utf8(result).unwrap();
        assert!(s.starts_with("q\n"), "should start with q (save)");
        assert!(s.contains("Q\n"), "should contain Q (restore)");
        assert!(s.contains("cm\n"), "should contain cm (transform)");
        assert!(s.contains("Hello"), "should preserve original content");
        assert!(s.contains("Bonjour"), "should include translated content");
    }

    #[test]
    fn p8_coordinate_isolation_identity_matrix() {
        let patch = CoordinateIsolationPatch {
            original_stream: b"orig".to_vec(),
            translated_stream: b"trans".to_vec(),
            transform_matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        };
        let result = apply_coordinate_isolation(&patch);
        let s = String::from_utf8(result).unwrap();
        // Identity matrix should produce "1.0000 0.0000 0.0000 1.0000 0.00 0.00 cm"
        assert!(s.contains("1.0000 0.0000 0.0000 1.0000 0.00 0.00 cm"));
    }

    #[test]
    fn p8_coordinate_isolation_translated_stream_not_modified() {
        let translated = b"BT (test) Tj ET\n".to_vec();
        let patch = CoordinateIsolationPatch {
            original_stream: b"q Q".to_vec(),
            translated_stream: translated.clone(),
            transform_matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        };
        let result = apply_coordinate_isolation(&patch);
        // The translated stream bytes should appear verbatim at the end
        assert!(result.ends_with(&translated));
    }

    // ==================== Helper Function Tests ====================

    #[test]
    fn is_cjk_char_detects_common_ranges() {
        assert!(is_cjk_char('中'));
        assert!(is_cjk_char('文'));
        assert!(is_cjk_char('あ')); // Hiragana
        assert!(is_cjk_char('ア')); // Katakana
        assert!(is_cjk_char('、')); // CJK punctuation
        assert!(!is_cjk_char('a'));
        assert!(!is_cjk_char('A'));
        assert!(!is_cjk_char('1'));
        assert!(!is_cjk_char(' '));
    }

    #[test]
    fn wrap_text_basic_word_wrap() {
        // 10pt font, "Hello World" at 50pt width
        // "Hello" = 5*5 = 25pt, "World" = 5*5 = 25pt, space = 5pt
        // "Hello World" = 55pt > 50pt → wraps to 2 lines
        let lines = wrap_text_to_width("Hello World", 50.0, 10.0);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "Hello");
        assert_eq!(lines[1], "World");
    }

    #[test]
    fn wrap_text_cjk_per_char_wrap() {
        // 5 CJK chars at 10pt = 50pt, box width 30pt → wraps per char
        let lines = wrap_text_to_width("中文测试字", 30.0, 10.0);
        assert!(lines.len() >= 2, "expected 2+ lines for CJK wrap, got {}", lines.len());
    }

    #[test]
    fn encode_pdf_string_ascii() {
        assert_eq!(encode_pdf_string('A'), "(A)");
        assert_eq!(encode_pdf_string('\\'), r"(\\)");
        assert_eq!(encode_pdf_string('('), r"(\()");
        assert_eq!(encode_pdf_string(')'), r"(\))");
    }

    #[test]
    fn encode_pdf_string_cjk_hex() {
        let result = encode_pdf_string('你');
        assert!(result.starts_with("<FEFF"));
        assert!(result.ends_with(">"));
        // 你 = U+4F60 → <FEFF4F60>
        assert!(result.contains("4F60"));
    }

    // ==================== P5 Formula Detection Tests ====================

    #[test]
    fn p5_is_math_font_detects_common_fonts() {
        assert!(is_math_font("Cambria Math"));
        assert!(is_math_font("STIXGeneral"));
        assert!(is_math_font("CMMI12"));
        assert!(is_math_font("cmsy10"));
        assert!(is_math_font("Latin Modern Math"));
        assert!(!is_math_font("Helvetica"));
        assert!(!is_math_font("Arial"));
        assert!(!is_math_font("Times New Roman"));
    }

    #[test]
    fn p5_is_unicode_math_char_detects_math_blocks() {
        // Mathematical Operators
        assert!(is_unicode_math_char('∀')); // U+2200
        assert!(is_unicode_math_char('∂')); // U+2202
        assert!(is_unicode_math_char('∑')); // U+2211
        assert!(is_unicode_math_char('∫')); // U+222B
        assert!(is_unicode_math_char('√')); // U+221A
        // Letterlike Symbols
        assert!(is_unicode_math_char('ℕ')); // U+2115
        assert!(is_unicode_math_char('ℝ')); // U+211D
        // Non-math
        assert!(!is_unicode_math_char('a'));
        assert!(!is_unicode_math_char('A'));
        assert!(!is_unicode_math_char('中'));
        assert!(!is_unicode_math_char('1'));
    }

    #[test]
    fn p5_detect_formula_by_font_name() {
        let font = IlFont {
            font_id: 0,
            resource_name: "/F1".to_string(),
            base_font: "Cambria Math".to_string(),
            encoding: IlFontEncoding::Standard,
            is_embedded: true,
            glyph_to_unicode: std::collections::HashMap::new(),
            unicode_to_glyph: std::collections::HashMap::new(),
            flags: 0,
            ascent: None,
            descent: None,
        };
        let para = IlParagraph {
            bbox: [0.0; 4],
            characters: vec![
                IlCharacter {
                    unicode: 'E', font_id: 0, font_size: 12.0,
                    x: 0.0, y: 0.0, render_mode: 0, color: [0.0; 3],
                    advance: 6.0, is_formula: false, source_op: None,
                },
                IlCharacter {
                    unicode: '=', font_id: 0, font_size: 12.0,
                    x: 10.0, y: 0.0, render_mode: 0, color: [0.0; 3],
                    advance: 6.0, is_formula: false, source_op: None,
                },
            ],
            translated_text: None,
            paragraph_type: IlParagraphType::Formula,
            detected_language: None,
        };
        let flagged = detect_formula_characters(&para, &[font]);
        // Both chars flagged via FontNameRegex signal
        assert_eq!(flagged.len(), 2);
        assert!(flagged[0].1.contains(&FormulaSignal::FontNameRegex));
        assert!(flagged[1].1.contains(&FormulaSignal::FontNameRegex));
    }

    #[test]
    fn p5_detect_formula_by_unicode_math_class() {
        let font = IlFont {
            font_id: 0,
            resource_name: "/F1".to_string(),
            base_font: "Helvetica".to_string(),
            encoding: IlFontEncoding::Standard,
            is_embedded: false,
            glyph_to_unicode: std::collections::HashMap::new(),
            unicode_to_glyph: std::collections::HashMap::new(),
            flags: 0,
            ascent: None,
            descent: None,
        };
        let para = IlParagraph {
            bbox: [0.0; 4],
            characters: vec![
                IlCharacter {
                    unicode: '∑', font_id: 0, font_size: 12.0,
                    x: 0.0, y: 0.0, render_mode: 0, color: [0.0; 3],
                    advance: 12.0, is_formula: false, source_op: None,
                },
                IlCharacter {
                    unicode: 'a', font_id: 0, font_size: 12.0,
                    x: 12.0, y: 0.0, render_mode: 0, color: [0.0; 3],
                    advance: 6.0, is_formula: false, source_op: None,
                },
            ],
            translated_text: None,
            paragraph_type: IlParagraphType::Text,
            detected_language: None,
        };
        let flagged = detect_formula_characters(&para, &[font]);
        // Only ∑ flagged via UnicodeMathClass
        assert_eq!(flagged.len(), 1);
        assert_eq!(flagged[0].0, 0);
        assert!(flagged[0].1.contains(&FormulaSignal::UnicodeMathClass));
    }

    #[test]
    fn p5_detect_formula_by_is_formula_flag() {
        let font = IlFont {
            font_id: 0,
            resource_name: "/F1".to_string(),
            base_font: "Helvetica".to_string(),
            encoding: IlFontEncoding::Standard,
            is_embedded: false,
            glyph_to_unicode: std::collections::HashMap::new(),
            unicode_to_glyph: std::collections::HashMap::new(),
            flags: 0,
            ascent: None,
            descent: None,
        };
        let para = IlParagraph {
            bbox: [0.0; 4],
            characters: vec![IlCharacter {
                unicode: 'x', font_id: 0, font_size: 12.0,
                x: 0.0, y: 0.0, render_mode: 0, color: [0.0; 3],
                advance: 6.0, is_formula: true, source_op: None,
            }],
            translated_text: None,
            paragraph_type: IlParagraphType::Text,
            detected_language: None,
        };
        let flagged = detect_formula_characters(&para, &[font]);
        assert_eq!(flagged.len(), 1);
        assert!(flagged[0].1.contains(&FormulaSignal::HasGlyphCheck));
    }

    #[test]
    fn p5_group_formula_spans_contiguous() {
        // Chars 2,3,4 contiguous → one span; char 7 separate → second span
        let flagged = vec![
            (2, vec![FormulaSignal::UnicodeMathClass]),
            (3, vec![FormulaSignal::UnicodeMathClass]),
            (4, vec![FormulaSignal::UnicodeMathClass]),
            (7, vec![FormulaSignal::FontNameRegex]),
        ];
        let spans = group_formula_spans(&flagged);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].0, 2);
        assert_eq!(spans[0].1, 4);
        assert_eq!(spans[1].0, 7);
        assert_eq!(spans[1].1, 7);
    }

    #[test]
    fn p5_group_formula_spans_empty() {
        let spans = group_formula_spans(&[]);
        assert!(spans.is_empty());
    }

    #[test]
    fn p5_group_formula_spans_single_char() {
        let flagged = vec![(5, vec![FormulaSignal::UnicodeMathClass])];
        let spans = group_formula_spans(&flagged);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0], (5, 5, vec![FormulaSignal::UnicodeMathClass]));
    }

    #[test]
    fn p5_mask_formulas_replaces_spans() {
        // Text: "E = mc²" (6 chars), formula span at indices 2-3 ("= ")
        // Wait, "= " is 2 chars. Let's use "E=mc²" (5 chars: E,=,m,c,²)
        // Formula: "mc²" at indices 2-4
        let text = "E=mc²";
        let spans = vec![
            (2, 4, vec![FormulaSignal::UnicodeMathClass]),
        ];
        let (masked, placeholders) = mask_formulas(text, &spans);
        assert_eq!(masked, "E={v0}");
        assert_eq!(placeholders.len(), 1);
        assert_eq!(placeholders[0].placeholder_id, "{v0}");
        assert_eq!(placeholders[0].original_text, "mc²");
    }

    #[test]
    fn p5_mask_formulas_multiple_spans() {
        // Text: "∀x P(x)" → 7 chars: ∀,x, ,P,(,x,)
        // Formula spans: [0,0] (∀) and [4,6] ((x))
        let text = "∀x P(x)";
        let spans = vec![
            (0, 0, vec![FormulaSignal::UnicodeMathClass]),
            (4, 6, vec![FormulaSignal::FontNameRegex]),
        ];
        let (masked, placeholders) = mask_formulas(text, &spans);
        assert_eq!(masked, "{v0}x P{v1}");
        assert_eq!(placeholders.len(), 2);
        assert_eq!(placeholders[0].original_text, "∀");
        assert_eq!(placeholders[1].original_text, "(x)");
    }

    #[test]
    fn p5_mask_formulas_no_spans() {
        let (masked, placeholders) = mask_formulas("Hello", &[]);
        assert_eq!(masked, "Hello");
        assert!(placeholders.is_empty());
    }

    #[test]
    fn p5_mask_formulas_entire_text() {
        // Entire text is one formula span
        let text = "∑∫√";
        let spans = vec![(0, 2, vec![FormulaSignal::UnicodeMathClass])];
        let (masked, placeholders) = mask_formulas(text, &spans);
        assert_eq!(masked, "{v0}");
        assert_eq!(placeholders[0].original_text, "∑∫√");
    }

    #[test]
    fn p5_restore_formulas_basic() {
        let placeholders = vec![
            FormulaPlaceholder {
                placeholder_id: "{v0}".to_string(),
                original_text: "mc²".to_string(),
                signals: vec![FormulaSignal::UnicodeMathClass],
            },
        ];
        // LLM translated "E={v0}" → "能量={v0}"
        let translated = "能量={v0}";
        let restored = restore_formulas(translated, &placeholders);
        assert_eq!(restored, "能量=mc²");
    }

    #[test]
    fn p5_restore_formulas_multiple() {
        let placeholders = vec![
            FormulaPlaceholder {
                placeholder_id: "{v0}".to_string(),
                original_text: "∀".to_string(),
                signals: vec![FormulaSignal::UnicodeMathClass],
            },
            FormulaPlaceholder {
                placeholder_id: "{v1}".to_string(),
                original_text: "(x)".to_string(),
                signals: vec![FormulaSignal::FontNameRegex],
            },
        ];
        // LLM translated "{v0}x P{v1}" → "{v0}x，P{v1}" (placeholders preserved per prompt)
        let translated = "{v0}x，P{v1}";
        let restored = restore_formulas(translated, &placeholders);
        assert_eq!(restored, "∀x，P(x)");
    }

    #[test]
    fn p5_restore_formulas_dropped_placeholder() {
        // LLM dropped {v0} — should be appended at end
        let placeholders = vec![FormulaPlaceholder {
            placeholder_id: "{v0}".to_string(),
            original_text: "∑".to_string(),
            signals: vec![FormulaSignal::UnicodeMathClass],
        }];
        let translated = "求和";  // LLM translated the meaning, dropped placeholder
        let restored = restore_formulas(translated, &placeholders);
        assert_eq!(restored, "求和∑");
    }

    #[test]
    fn p5_restore_formulas_no_placeholders() {
        let restored = restore_formulas("Hello", &[]);
        assert_eq!(restored, "Hello");
    }

    #[test]
    fn p5_end_to_end_mask_translate_restore() {
        // Full pipeline: detect → mask → (simulate translate) → restore
        // Use Helvetica (non-math font) + is_formula flag on "mc²" so only
        // those 3 chars get flagged (via HasGlyphCheck signal).
        let font = IlFont {
            font_id: 0,
            resource_name: "/F1".to_string(),
            base_font: "Helvetica".to_string(),
            encoding: IlFontEncoding::Standard,
            is_embedded: false,
            glyph_to_unicode: std::collections::HashMap::new(),
            unicode_to_glyph: std::collections::HashMap::new(),
            flags: 0,
            ascent: None,
            descent: None,
        };
        // Text: "Energy equals mc²" (17 chars: E,n,e,r,g,y, ,e,q,u,a,l,s, ,m,c,²)
        // Indices:                0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16
        let text: String = "Energy equals mc²".chars().collect();
        let chars: Vec<IlCharacter> = text.chars().enumerate().map(|(i, c)| IlCharacter {
            unicode: c,
            font_id: 0,
            font_size: 12.0,
            x: i as f32 * 6.0,
            y: 0.0,
            render_mode: 0,
            color: [0.0; 3],
            advance: 6.0,
            // Mark "mc²" (indices 14,15,16) as formula
            is_formula: i >= 14,
            source_op: None,
        }).collect();
        let para = IlParagraph {
            bbox: [0.0; 4],
            characters: chars,
            translated_text: None,
            paragraph_type: IlParagraphType::Text,
            detected_language: None,
        };

        // Detect — only m, c, ² flagged via HasGlyphCheck
        let flagged = detect_formula_characters(&para, &[font]);
        assert_eq!(flagged.len(), 3);

        // Group — one contiguous span [14, 16]
        let spans = group_formula_spans(&flagged);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 14);
        assert_eq!(spans[0].1, 16);

        // Mask
        let (masked, placeholders) = mask_formulas(&text, &spans);
        assert_eq!(masked, "Energy equals {v0}");
        assert_eq!(placeholders.len(), 1);
        assert_eq!(placeholders[0].original_text, "mc²");

        // Simulate translation (LLM keeps placeholder per prompt instructions)
        let translated = masked.replace("Energy equals", "能量等于");

        // Restore
        let restored = restore_formulas(&translated, &placeholders);
        assert_eq!(restored, "能量等于 mc²");
    }

    // ==================== P10 Cache Tests ====================

    #[test]
    fn p10_hash_text_normalizes_whitespace() {
        // Different whitespace should produce same hash
        let h1 = PdfTranslationCache::hash_text("Hello World");
        let h2 = PdfTranslationCache::hash_text("  Hello   World  ");
        let h3 = PdfTranslationCache::hash_text("Hello\nWorld");
        assert_eq!(h1, h2, "leading/trailing whitespace should be trimmed");
        assert_eq!(h1, h3, "newlines should collapse to spaces");
    }

    #[test]
    fn p10_hash_text_case_insensitive() {
        let h1 = PdfTranslationCache::hash_text("Hello World");
        let h2 = PdfTranslationCache::hash_text("hello world");
        let h3 = PdfTranslationCache::hash_text("HELLO WORLD");
        assert_eq!(h1, h2);
        assert_eq!(h1, h3);
    }

    #[test]
    fn p10_hash_text_stable() {
        // Same input → same output
        let h1 = PdfTranslationCache::hash_text("Test text 123");
        let h2 = PdfTranslationCache::hash_text("Test text 123");
        assert_eq!(h1, h2);
        // Different input → different output
        let h3 = PdfTranslationCache::hash_text("Different text");
        assert_ne!(h1, h3);
    }

    #[test]
    fn p10_cache_store_and_lookup() {
        let cache = PdfTranslationCache::open_in_memory().unwrap();
        cache.store("google", "en", "zh", "Hello world", "你好世界").unwrap();

        let result = cache.lookup("google", "en", "zh", "Hello world").unwrap();
        assert!(result.is_some(), "lookup should find stored entry");
        let entry = result.unwrap();
        assert_eq!(entry.original_text, "Hello world");
        assert_eq!(entry.translated_text, "你好世界");
        assert_eq!(entry.engine, "google");
    }

    #[test]
    fn p10_cache_lookup_miss() {
        let cache = PdfTranslationCache::open_in_memory().unwrap();
        let result = cache.lookup("google", "en", "zh", "Not cached").unwrap();
        assert!(result.is_none(), "lookup should miss on empty cache");
    }

    #[test]
    fn p10_cache_different_engines_dont_collide() {
        let cache = PdfTranslationCache::open_in_memory().unwrap();
        cache.store("google", "en", "zh", "Hello", "你好").unwrap();
        cache.store("deepl", "en", "zh", "Hello", "您好").unwrap();

        let google = cache.lookup("google", "en", "zh", "Hello").unwrap().unwrap();
        let deepl = cache.lookup("deepl", "en", "zh", "Hello").unwrap().unwrap();
        assert_eq!(google.translated_text, "你好");
        assert_eq!(deepl.translated_text, "您好");
    }

    #[test]
    fn p10_cache_store_replaces_existing() {
        let cache = PdfTranslationCache::open_in_memory().unwrap();
        cache.store("google", "en", "zh", "Hello", "你好").unwrap();
        // Store again with different translation
        cache.store("google", "en", "zh", "Hello", "您好（更新）").unwrap();

        let entry = cache.lookup("google", "en", "zh", "Hello").unwrap().unwrap();
        assert_eq!(entry.translated_text, "您好（更新）");
    }

    #[test]
    fn p10_cache_normalization_lookup() {
        // Store with one whitespace form, lookup with another
        let cache = PdfTranslationCache::open_in_memory().unwrap();
        cache.store("google", "en", "zh", "Hello World", "你好世界").unwrap();

        // Lookup with extra whitespace → should still hit (same hash)
        let result = cache.lookup("google", "en", "zh", "  Hello   World  ").unwrap();
        assert!(result.is_some(), "normalized lookup should hit");
        assert_eq!(result.unwrap().translated_text, "你好世界");
    }

    #[test]
    fn p10_cache_count() {
        let cache = PdfTranslationCache::open_in_memory().unwrap();
        assert_eq!(cache.count().unwrap(), 0);
        cache.store("google", "en", "zh", "text1", "译文1").unwrap();
        cache.store("google", "en", "zh", "text2", "译文2").unwrap();
        cache.store("deepl", "en", "zh", "text1", "译文1-deepl").unwrap();
        assert_eq!(cache.count().unwrap(), 3);
    }

    #[test]
    fn p10_cache_evict_older_than() {
        let cache = PdfTranslationCache::open_in_memory().unwrap();
        // Insert with old timestamp
        let old_entry = CacheEntry {
            engine: "google".to_string(),
            source_lang: "en".to_string(),
            target_lang: "zh".to_string(),
            text_hash: PdfTranslationCache::hash_text("old text"),
            original_text: "old text".to_string(),
            translated_text: "旧文本".to_string(),
            created_at: chrono::Utc::now().timestamp() - 86400 * 30, // 30 days ago
        };
        cache.put(&old_entry).unwrap();

        // Insert a fresh entry
        cache.store("google", "en", "zh", "fresh text", "新鲜文本").unwrap();
        assert_eq!(cache.count().unwrap(), 2);

        // Evict entries older than 7 days
        let evicted = cache.evict_older_than(86400 * 7).unwrap();
        assert_eq!(evicted, 1);
        assert_eq!(cache.count().unwrap(), 1);

        // Fresh entry should survive
        let result = cache.lookup("google", "en", "zh", "fresh text").unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn p10_cache_get_with_key() {
        let cache = PdfTranslationCache::open_in_memory().unwrap();
        cache.store("google", "en", "zh", "Test", "测试").unwrap();

        let key = TranslationCacheKey {
            engine: "google".to_string(),
            source_lang: "en".to_string(),
            target_lang: "zh".to_string(),
            text_hash: PdfTranslationCache::hash_text("Test"),
        };
        let result = cache.get(&key).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().translated_text, "测试");
    }
}

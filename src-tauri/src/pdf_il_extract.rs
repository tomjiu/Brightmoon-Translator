//! P2: PDF → IL extraction frontend using pdfium-render.
//!
//! Binds to Google's Pdfium C++ library at runtime to extract per-character
//! positions, fonts, and encodings from PDF files. The extracted `IlDocument`
//! feeds into the P3/P4/P5/P8 reflow + writeback pipeline for high-fidelity
//! bilingual PDF generation.
//!
//! Coordinate system: PDF origin is bottom-left, y grows upward. We preserve
//! this in IL (matching `pdf_il.rs` convention).

use crate::pdf_il::{
    IlCharacter, IlDocument, IlFont, IlFontEncoding, IlMetadata, IlPage, IlParagraph,
    IlParagraphType,
};
use pdfium_render::prelude::*;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Thread-safe singleton Pdfium binding. Pdfium is NOT thread-safe internally,
/// so all extraction must happen on a single thread (we use `spawn_blocking`).
static PDFIUM: OnceLock<Option<Pdfium>> = OnceLock::new();

/// Initialize the Pdfium library. Tries (in order):
/// 1. Library in the same directory as the executable
/// 2. System-provided library
///
/// Returns an error string if neither is available.
fn get_pdfium() -> Result<&'static Pdfium, String> {
    let pdfium = PDFIUM.get_or_init(|| {
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_system_library())
            .ok()
            .map(Pdfium::new)
    });

    pdfium.as_ref().ok_or_else(|| {
        "Pdfium library not found. Place pdfium.dll next to the executable or install it system-wide.".to_string()
    })
}

/// P2: Extract an `IlDocument` from a PDF file.
///
/// Opens the PDF, iterates all pages, extracts per-character data (Unicode,
/// position, font, size), clusters characters into paragraphs, and returns
/// a fully populated `IlDocument` ready for translation and writeback.
pub fn extract_il_from_pdf(file_path: &str) -> Result<IlDocument, String> {
    let pdfium = get_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(file_path, None)
        .map_err(|e| format!("Failed to open PDF '{file_path}': {e}"))?;

    let total_pages = document.pages().len() as usize;
    tracing::info!("[P2] Extracting IL from PDF: {} pages", total_pages);

    let mut il_pages = Vec::with_capacity(total_pages);
    let mut global_fonts: Vec<IlFont> = Vec::new();
    let mut font_name_map: HashMap<String, u32> = HashMap::new();

    for (page_idx, page) in document.pages().iter().enumerate() {
        let il_page = extract_page(
            &page,
            page_idx + 1, // 1-indexed
            &mut global_fonts,
            &mut font_name_map,
        )?;
        il_pages.push(il_page);
    }

    Ok(IlDocument {
        pages: il_pages,
        fonts: global_fonts,
        metadata: IlMetadata {
            title: None,
            author: None,
            subject: None,
            keywords: None,
            creator: None,
            producer: None,
            creation_date: None,
            mod_date: None,
        },
        pdf_version: "1.7".to_string(),
    })
}

/// Extract IL data from a single `PdfPage`.
fn extract_page(
    page: &PdfPage,
    page_number: usize,
    global_fonts: &mut Vec<IlFont>,
    font_name_map: &mut HashMap<String, u32>,
) -> Result<IlPage, String> {
    let page_width = page.width().value;
    let page_height = page.height().value;

    let text = page.text()
        .map_err(|e| format!("Failed to extract text from page {page_number}: {e}"))?;
    let mut characters: Vec<IlCharacter> = Vec::new();
    let mut raw_chars: Vec<RawChar> = Vec::new();

    for char_obj in text.chars().iter() {
        // Skip characters with no Unicode (e.g. glyph-only fonts without ToUnicode CMap)
        let unicode = match char_obj.unicode_char() {
            Some(c) if c != '\0' => c,
            _ => continue,
        };

        let font_name = char_obj.font_name();
        let font_size = char_obj.scaled_font_size().value;

        // Get or create font ID
        let font_id = *font_name_map
            .entry(font_name.clone())
            .or_insert_with(|| {
                let id = global_fonts.len() as u32;
                global_fonts.push(IlFont {
                    font_id: id,
                    resource_name: format!("/F{}", id + 1),
                    base_font: font_name.clone(),
                    encoding: IlFontEncoding::Standard,
                    is_embedded: true, // pdfium handles font substitution; assume available
                    glyph_to_unicode: HashMap::new(),
                    unicode_to_glyph: HashMap::new(),
                    flags: 0,
                    ascent: None,
                    descent: None,
                });
                id
            });

        // Character position (PDF coordinate system: origin at bottom-left)
        let (x, y) = match char_obj.origin() {
            Ok((px, py)) => (px.value, py.value),
            Err(_) => {
                // Fallback: use loose bounds center
                continue;
            }
        };

        // Character bounding box via loose_bounds
        let (width, _height) = match char_obj.loose_bounds() {
            Ok(rect) => (rect.width().value, rect.height().value),
            Err(_) => (font_size * 0.5, font_size),
        };

        // Render mode
        let render_mode = match char_obj.render_mode() {
            Ok(mode) => mode as u8,
            Err(_) => 0,
        };

        // Fill color
        let color = match char_obj.fill_color() {
            Ok(c) => {
                [
                    f32::from(c.red()) / 255.0,
                    f32::from(c.green()) / 255.0,
                    f32::from(c.blue()) / 255.0,
                ]
            }
            Err(_) => [0.0, 0.0, 0.0],
        };

        // Advance width (estimated from char width)
        let advance = width;

        // Check if char is likely a formula (symbolic font or math Unicode)
        let is_formula = is_likely_formula_char(&unicode, &font_name);

        let il_char = IlCharacter {
            unicode,
            font_id,
            font_size,
            x,
            y,
            render_mode,
            color,
            advance,
            is_formula,
            source_op: None,
        };

        characters.push(il_char);
        raw_chars.push(RawChar {
            x,
            y,
            font_size,
            char_idx: characters.len() - 1,
        });
    }

    // Cluster characters into paragraphs
    let paragraphs = cluster_characters_into_paragraphs(&characters, &raw_chars, page_width, page_height);

    tracing::info!(
        "[P2] Page {}: {} chars → {} paragraphs",
        page_number,
        characters.len(),
        paragraphs.len()
    );

    Ok(IlPage {
        page_number,
        width: page_width,
        height: page_height,
        media_box: [0.0, 0.0, page_width, page_height],
        rotation: 0,
        paragraphs,
        vector_ops: Vec::new(),
        images: Vec::new(),
    })
}

/// Raw character data used for clustering (avoids borrowing `IlCharacter`).
struct RawChar {
    x: f32,
    y: f32,
    font_size: f32,
    char_idx: usize,
}

/// Check if a character is likely part of a formula based on Unicode block
/// and font name. This is a heuristic; P5 does more precise detection.
fn is_likely_formula_char(c: &char, font_name: &str) -> bool {
    // Math Unicode blocks
    let code = *c as u32;
    if matches!(code,
        0x2200..=0x22FF | 0x27C0..=0x27EF | 0x2980..=0x29FF
        | 0x2A00..=0x2AFF | 0x2100..=0x214F | 0x1D400..=0x1D7FF
    ) {
        return true;
    }
    // Symbolic font
    let lower = font_name.to_lowercase();
    if lower.contains("symbol") || lower.contains("math") || lower.contains("cmmi") || lower.contains("cmsy") {
        return true;
    }
    false
}

/// Cluster characters into paragraphs based on spatial proximity.
///
/// Algorithm:
/// 1. Sort characters by (y descending, x ascending) — top-to-bottom, left-to-right
/// 2. Group into lines: same y (within tolerance = `font_size` * 0.3)
/// 3. Group lines into paragraphs: consecutive lines with similar x-start
///    and line spacing < 1.5x `font_size`
fn cluster_characters_into_paragraphs(
    characters: &[IlCharacter],
    raw_chars: &[RawChar],
    _page_width: f32,
    _page_height: f32,
) -> Vec<IlParagraph> {
    if raw_chars.is_empty() {
        return Vec::new();
    }

    // Sort by (y descending, x ascending) — reading order
    let mut sorted: Vec<usize> = (0..raw_chars.len()).collect();
    sorted.sort_by(|&a, &b| {
        let ra = &raw_chars[a];
        let rb = &raw_chars[b];
        // Group by y (descending), then x (ascending)
        let y_tol = ra.font_size.max(rb.font_size) * 0.3;
        if (ra.y - rb.y).abs() > y_tol {
            rb.y.partial_cmp(&ra.y).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            ra.x.partial_cmp(&rb.x).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // Group into lines
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut current_line: Vec<usize> = Vec::new();
    let mut current_y: Option<f32> = None;
    let mut current_font_size: f32 = 10.0;

    for &raw_idx in &sorted {
        let rc = &raw_chars[raw_idx];
        match current_y {
            None => {
                current_y = Some(rc.y);
                current_font_size = rc.font_size;
                current_line.push(raw_idx);
            }
            Some(cy) => {
                let tol = current_font_size.max(rc.font_size) * 0.3;
                if (rc.y - cy).abs() <= tol {
                    current_line.push(raw_idx);
                } else {
                    lines.push(std::mem::take(&mut current_line));
                    current_y = Some(rc.y);
                    current_font_size = rc.font_size;
                    current_line.push(raw_idx);
                }
            }
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    // Group lines into paragraphs
    let mut paragraphs: Vec<IlParagraph> = Vec::new();
    let mut para_line_indices: Vec<Vec<usize>> = Vec::new();
    let mut para_x_start: Option<f32> = None;
    let mut para_y_top: Option<f32> = None;
    let mut para_font_size: f32 = 10.0;

    for line in &lines {
        if line.is_empty() {
            continue;
        }
        let line_x_start = raw_chars[line[0]].x;
        let line_y = raw_chars[line[0]].y;
        let line_fs = raw_chars[line[0]].font_size;

        let should_split = match (para_y_top, para_x_start) {
            (None, None) => false,
            (Some(py), Some(px)) => {
                // Split if: large vertical gap (> 1.5x font size) OR x-start shifts significantly
                let v_gap = py - line_y; // previous y was higher (descending order)
                let x_shift = (line_x_start - px).abs();
                v_gap > para_font_size * 1.5 || x_shift > para_font_size * 3.0
            }
            _ => false,
        };

        if should_split {
            // Flush current paragraph
            if !para_line_indices.is_empty() {
                if let Some(p) = build_paragraph(&para_line_indices, raw_chars, characters) {
                    paragraphs.push(p);
                }
            }
            para_line_indices.clear();
        }

        para_line_indices.push(line.clone());
        para_x_start = Some(line_x_start);
        para_y_top = Some(line_y);
        para_font_size = line_fs;
    }

    // Flush last paragraph
    if !para_line_indices.is_empty() {
        if let Some(p) = build_paragraph(&para_line_indices, raw_chars, characters) {
            paragraphs.push(p);
        }
    }

    paragraphs
}

/// Build an `IlParagraph` from a group of line indices.
fn build_paragraph(
    line_indices: &[Vec<usize>],
    raw_chars: &[RawChar],
    characters: &[IlCharacter],
) -> Option<IlParagraph> {
    if line_indices.is_empty() {
        return None;
    }

    // Collect all char indices in reading order
    let mut char_indices: Vec<usize> = Vec::new();
    for line in line_indices {
        for &raw_idx in line {
            char_indices.push(raw_chars[raw_idx].char_idx);
        }
    }

    if char_indices.is_empty() {
        return None;
    }

    // Compute bounding box from all characters
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for &idx in &char_indices {
        let ch = &characters[idx];
        min_x = min_x.min(ch.x);
        min_y = min_y.min(ch.y);
        max_x = max_x.max(ch.x + ch.advance);
        max_y = max_y.max(ch.y + ch.font_size);
    }

    // Build paragraph text from characters
    let mut para_text = String::new();
    let mut prev_idx: Option<usize> = None;
    for &idx in &char_indices {
        let ch = &characters[idx];
        if let Some(pi) = prev_idx {
            let prev = &characters[pi];
            // Insert space if horizontal gap is large
            let gap = ch.x - (prev.x + prev.advance);
            if gap > prev.font_size * 0.3 && !para_text.ends_with(' ') {
                para_text.push(' ');
            }
            // Insert newline if y changed significantly
            if (ch.y - prev.y).abs() > prev.font_size * 0.5 && !para_text.is_empty() {
                para_text.push('\n');
            }
        }
        para_text.push(ch.unicode);
        prev_idx = Some(idx);
    }

    // Detect paragraph type (heuristic)
    let para_type = detect_paragraph_type(&para_text, &characters[char_indices[0]]);

    Some(IlParagraph {
        bbox: [min_x, min_y, max_x, max_y],
        characters: char_indices.into_iter().map(|i| characters[i].clone()).collect(),
        translated_text: None,
        paragraph_type: para_type,
        detected_language: None,
    })
}

/// Heuristic paragraph type detection.
fn detect_paragraph_type(text: &str, first_char: &IlCharacter) -> IlParagraphType {
    // Title: short text, large font
    let line_count = text.lines().count();
    let char_count = text.chars().count();
    if char_count < 80 && line_count <= 2 && first_char.font_size >= 14.0 {
        return IlParagraphType::Heading;
    }
    // Formula: if first char is flagged as formula
    if first_char.is_formula {
        return IlParagraphType::Formula;
    }
    IlParagraphType::Text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_likely_formula_char_math_unicode() {
        assert!(is_likely_formula_char(&'∑', "Helvetica"));
        assert!(is_likely_formula_char(&'∫', "Helvetica"));
        assert!(is_likely_formula_char(&'∀', "Helvetica"));
        assert!(!is_likely_formula_char(&'a', "Helvetica"));
        assert!(!is_likely_formula_char(&'A', "Helvetica"));
        assert!(!is_likely_formula_char(&'中', "Helvetica"));
    }

    #[test]
    fn test_is_likely_formula_char_math_font() {
        assert!(is_likely_formula_char(&'x', "CMMI12"));
        assert!(is_likely_formula_char(&'x', "CMSY10"));
        assert!(is_likely_formula_char(&'x', "Symbol"));
        assert!(is_likely_formula_char(&'x', "Cambria Math"));
        assert!(!is_likely_formula_char(&'x', "Helvetica"));
        assert!(!is_likely_formula_char(&'x', "Arial"));
    }

    #[test]
    fn test_cluster_empty_input() {
        let characters: Vec<IlCharacter> = Vec::new();
        let raw_chars: Vec<RawChar> = Vec::new();
        let paras = cluster_characters_into_paragraphs(&characters, &raw_chars, 612.0, 792.0);
        assert!(paras.is_empty());
    }

    #[test]
    fn test_cluster_single_line() {
        // 5 chars on the same y, increasing x → 1 paragraph, 1 line
        let characters: Vec<IlCharacter> = (0..5)
            .map(|i| IlCharacter {
                unicode: ('A' as u8 + i as u8) as char,
                font_id: 0,
                font_size: 12.0,
                x: 72.0 + i as f32 * 6.6,
                y: 700.0,
                render_mode: 0,
                color: [0.0; 3],
                advance: 6.6,
                is_formula: false,
                source_op: None,
            })
            .collect();
        let raw_chars: Vec<RawChar> = (0..5)
            .map(|i| RawChar {
                x: 72.0 + i as f32 * 6.6,
                y: 700.0,
                font_size: 12.0,
                char_idx: i,
            })
            .collect();
        let paras = cluster_characters_into_paragraphs(&characters, &raw_chars, 612.0, 792.0);
        assert_eq!(paras.len(), 1);
        assert_eq!(paras[0].characters.len(), 5);
        // Text should be "ABCDE"
        let text: String = paras[0].characters.iter().map(|c| c.unicode).collect();
        assert_eq!(text, "ABCDE");
    }

    #[test]
    fn test_cluster_two_lines_one_paragraph() {
        // 3 chars on y=700, 3 chars on y=686 (close spacing) → 1 paragraph
        let mut characters = Vec::new();
        let mut raw_chars = Vec::new();
        for i in 0..3 {
            characters.push(IlCharacter {
                unicode: ('A' as u8 + i as u8) as char,
                font_id: 0, font_size: 12.0,
                x: 72.0 + i as f32 * 6.6, y: 700.0,
                render_mode: 0, color: [0.0; 3],
                advance: 6.6, is_formula: false, source_op: None,
            });
            raw_chars.push(RawChar { x: 72.0 + i as f32 * 6.6, y: 700.0, font_size: 12.0, char_idx: i });
        }
        for i in 0..3 {
            let idx = i + 3;
            characters.push(IlCharacter {
                unicode: ('D' as u8 + i as u8) as char,
                font_id: 0, font_size: 12.0,
                x: 72.0 + i as f32 * 6.6, y: 686.0,
                render_mode: 0, color: [0.0; 3],
                advance: 6.6, is_formula: false, source_op: None,
            });
            raw_chars.push(RawChar { x: 72.0 + i as f32 * 6.6, y: 686.0, font_size: 12.0, char_idx: idx });
        }
        let paras = cluster_characters_into_paragraphs(&characters, &raw_chars, 612.0, 792.0);
        assert_eq!(paras.len(), 1, "expected 1 paragraph, got {}", paras.len());
        assert_eq!(paras[0].characters.len(), 6);
    }

    #[test]
    fn test_cluster_two_paragraphs_large_gap() {
        // 3 chars on y=700, 3 chars on y=660 (large gap > 1.5*12=18) → 2 paragraphs
        let mut characters = Vec::new();
        let mut raw_chars = Vec::new();
        for i in 0..3 {
            characters.push(IlCharacter {
                unicode: ('A' as u8 + i as u8) as char,
                font_id: 0, font_size: 12.0,
                x: 72.0 + i as f32 * 6.6, y: 700.0,
                render_mode: 0, color: [0.0; 3],
                advance: 6.6, is_formula: false, source_op: None,
            });
            raw_chars.push(RawChar { x: 72.0 + i as f32 * 6.6, y: 700.0, font_size: 12.0, char_idx: i });
        }
        for i in 0..3 {
            let idx = i + 3;
            characters.push(IlCharacter {
                unicode: ('D' as u8 + i as u8) as char,
                font_id: 0, font_size: 12.0,
                x: 72.0 + i as f32 * 6.6, y: 660.0, // 40pt gap > 18pt
                render_mode: 0, color: [0.0; 3],
                advance: 6.6, is_formula: false, source_op: None,
            });
            raw_chars.push(RawChar { x: 72.0 + i as f32 * 6.6, y: 660.0, font_size: 12.0, char_idx: idx });
        }
        let paras = cluster_characters_into_paragraphs(&characters, &raw_chars, 612.0, 792.0);
        assert_eq!(paras.len(), 2, "expected 2 paragraphs, got {}", paras.len());
    }

    #[test]
    fn test_detect_paragraph_type_title() {
        let ch = IlCharacter {
            unicode: 'H', font_id: 0, font_size: 18.0,
            x: 72.0, y: 700.0, render_mode: 0, color: [0.0; 3],
            advance: 10.0, is_formula: false, source_op: None,
        };
        assert_eq!(detect_paragraph_type("Hello World", &ch), IlParagraphType::Heading);
    }

    #[test]
    fn test_detect_paragraph_type_text() {
        let ch = IlCharacter {
            unicode: 'T', font_id: 0, font_size: 10.0,
            x: 72.0, y: 700.0, render_mode: 0, color: [0.0; 3],
            advance: 5.0, is_formula: false, source_op: None,
        };
        let long_text = "This is a normal paragraph of text that is long enough to not be considered a title.";
        assert_eq!(detect_paragraph_type(long_text, &ch), IlParagraphType::Text);
    }

    #[test]
    fn test_detect_paragraph_type_formula() {
        let ch = IlCharacter {
            unicode: '∑', font_id: 0, font_size: 12.0,
            x: 72.0, y: 700.0, render_mode: 0, color: [0.0; 3],
            advance: 12.0, is_formula: true, source_op: None,
        };
        assert_eq!(detect_paragraph_type("∑∑", &ch), IlParagraphType::Formula);
    }
}

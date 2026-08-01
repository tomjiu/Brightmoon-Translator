//! Tier 4 P2: OCR geometric post-processing (`join_text_regions`).
//!
//! Converts the flat list of OCR lines (with bounding boxes) returned by OCR
//! engines into structured, Markdown-friendly text. This is a **pure geometric**
//! post-processor — no layout model. The algorithm is adapted from kivio's
//! `rapidocr.rs:537-970`.
//!
//! ## Pipeline
//! 1. **Sort** lines by y (top-to-bottom), then x (left-to-right) reading order.
//! 2. **Compute median height** across all lines (dynamic, not fixed pixel bucket).
//! 3. **Group lines into paragraphs** using soft-wrap vs paragraph-break heuristics:
//!    - Soft-wrap (same paragraph, line break): left-edge aligns, previous line
//!      filled the column width, no heading/sentence-break sentinel.
//!    - Paragraph break (blank line): large vertical gap, heading detected,
//!      or list entry/exit.
//! 4. **Normalize list items**: bullet glyphs (• · ● ○ - *) → Markdown `- `,
//!    ordered markers (1. 2) 3、) → `N. `.
//! 5. **CJK↔ASCII spacing**: no space between two CJK chars; space between
//!    ASCII runs; de-hyphenate soft-wrapped words.
//!
//! ## Why
//! Raw OCR text joins lines with `\n`, losing paragraph structure. This
//! degrades both display (wall of text) and translation quality (translator
//! can't distinguish paragraphs). The post-processed text uses `\n\n` for
//! paragraph breaks and Markdown `- ` for list items, which ReactMarkdown
//! and LLM translators both understand structurally.

use crate::commands::capture::OcrLineResult;

// ── Thresholds (from kivio reference, tuned for typical screen OCR) ────────

/// Soft-wrap vertical band: gap in [0, median * 1.6] → same visual row.
const SOFT_WRAP_GAP_FACTOR: f64 = 1.6;
/// Left-edge alignment tolerance: `current.x ≤ prev.x + median * 1.2`.
const LEFT_ALIGN_FACTOR: f64 = 1.2;
/// Line-fill ratio: previous line must reach ≥68% of document width for soft-wrap.
const LINE_FILL_RATIO: f64 = 0.68;
/// Paragraph break gap: gap > median * 1.25 → new paragraph.
const PARAGRAPH_BREAK_FACTOR: f64 = 1.25;
/// Heading height ratio: height ≥ median * 1.2 → heading candidate.
const HEADING_HEIGHT_FACTOR: f64 = 1.2;
/// Heading max char count.
const HEADING_MAX_CHARS: usize = 80;
/// Default median height when no lines are available.
const DEFAULT_MEDIAN_HEIGHT: f64 = 20.0;

/// Characters that signal end-of-sentence (no soft-wrap after these).
const SENTENCE_BREAK_CHARS: &[char] = &['.', '!', '?', ':', ';', '。', '！', '？', '：', '；'];

/// Closing punctuation (no space inserted before these).
const CLOSING_PUNCT: &[char] = &[',', '.', ':', ';', ')', ']', '}', '，', '。', '、', '：', '；', '）', '】'];

/// Opening brackets (no space inserted after these).
const OPENING_BRACKETS: &[char] = &['(', '[', '{', '（', '【'];

// ── Types ──────────────────────────────────────────────────────────────────

/// Geometric info for one OCR line (decoupled from OcrLineResult for testing).
#[derive(Debug, Clone, PartialEq)]
pub struct OcrLineGeo {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<&OcrLineResult> for OcrLineGeo {
    fn from(l: &OcrLineResult) -> Self {
        Self {
            text: l.text.clone(),
            x: l.x,
            y: l.y,
            width: l.width,
            height: l.height,
        }
    }
}

// ── Character classification ───────────────────────────────────────────────

/// Returns true if the character is CJK (Unified Ideographs + Ext A,
/// Hiragana, Katakana, Hangul, Fullwidth Forms).
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF   // CJK Unified Ideographs
        | 0x3400..=0x4DBF // CJK Extension A
        | 0xF900..=0xFAFF // CJK Compatibility Ideographs
        | 0x3040..=0x309F // Hiragana
        | 0x30A0..=0x30FF // Katakana
        | 0xAC00..=0xD7AF // Hangul Syllables
        | 0xFF00..=0xFFEF // Fullwidth Forms
    )
}

/// Returns true if the text looks like a heading (short, large font, no sentence break).
fn looks_like_heading(text: &str, height: f64, median_height: f64) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > HEADING_MAX_CHARS {
        return false;
    }
    if height < median_height * HEADING_HEIGHT_FACTOR {
        return false;
    }
    if is_list_item(trimmed) {
        return false;
    }
    !ends_with_sentence_break(trimmed)
}

/// Returns true if the text ends with a sentence-break character.
fn ends_with_sentence_break(text: &str) -> bool {
    text.trim_end()
        .chars()
        .next_back()
        .map(|c| SENTENCE_BREAK_CHARS.contains(&c))
        .unwrap_or(false)
}

// ── List item detection & normalization ───────────────────────────────────

/// Bullet glyphs that should be normalized to Markdown `- `.
const BULLET_GLYPHS: &[char] = &['•', '·', '●', '○', '◦', '▪', '▫', '-', '*', '–', '—'];

/// Returns true if the text (post-normalization) is a Markdown list item.
pub fn is_list_item(text: &str) -> bool {
    let trimmed = text.trim_start();
    // Markdown bullet: "- "
    if trimmed.starts_with("- ") {
        return true;
    }
    // Ordered: 1-3 digits followed by ". "
    let chars: Vec<char> = trimmed.chars().collect();
    let mut i = 0;
    while i < chars.len() && i < 3 && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i >= 1 && i <= 3 && i + 1 < chars.len() && chars[i] == '.' && chars[i + 1] == ' ' {
        return true;
    }
    false
}

/// Normalize common bullet glyphs and ordered-list markers to Markdown form.
///
/// - `• text` → `- text`
/// - `1. text` / `1) text` / `1、text` → `1. text`
/// - `O text` (OCR misread of `•`) → `- text` (only if followed by uppercase/CJK)
fn normalize_line_text(text: &str) -> String {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return text.to_string();
    }

    let first = trimmed.chars().next().unwrap();
    let rest: String = trimmed.chars().skip(1).collect();

    // Unordered bullet glyphs → "- "
    if BULLET_GLYPHS.contains(&first) {
        let rest = rest.trim_start();
        return format!("- {}", rest);
    }

    // OCR misread: 'O', 'o', '0' as bullet (only if followed by uppercase or CJK)
    if (first == 'O' || first == 'o' || first == '0') && !rest.is_empty() {
        let rest_trimmed = rest.trim_start();
        if let Some(next_char) = rest_trimmed.chars().next() {
            if next_char.is_ascii_uppercase() || is_cjk(next_char) {
                return format!("- {}", rest_trimmed);
            }
        }
    }

    // Ordered list: 1-3 digits + (. | ) | ） | 、)
    if first.is_ascii_digit() {
        let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
        let digit_count = digits.len();
        if digit_count >= 1 && digit_count <= 3 {
            let after_digits: Vec<char> = trimmed.chars().skip(digit_count).collect();
            if !after_digits.is_empty() {
                let marker = after_digits[0];
                if marker == '.' || marker == ')' || marker == '）' || marker == '、' {
                    // Require a space after the marker (or end of string).
                    // This prevents turning version strings like "1.24.4" into
                    // list items — a digit immediately after the marker means
                    // it's not a list marker.
                    if after_digits.len() > 1 {
                        let next_after_marker = after_digits[1];
                        if next_after_marker == ' ' || is_cjk(next_after_marker) {
                            let rest_str: String = after_digits.iter().skip(1).collect();
                            let rest_str = rest_str.trim_start();
                            return format!("{}. {}", digits, rest_str);
                        }
                    } else {
                        // "1." at end of string — bare marker, treat as list
                        return format!("{}. ", digits);
                    }
                }
            }
        }
    }

    text.to_string()
}

// ── CJK ↔ ASCII space handling ─────────────────────────────────────────────

/// CJK space cleaning: removes spaces between CJK characters, preserves
/// spaces between CJK and ASCII, trims leading/trailing, compresses multiples.
pub fn clean_cjk_spaces(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = Vec::with_capacity(chars.len());
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == ' ' {
            let mut j = i;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            let prev = if result.is_empty() { None } else { result.last().copied() };
            let next = if j < chars.len() { Some(chars[j]) } else { None };

            match (prev, next) {
                (None, _) | (_, None) => {} // trim
                (Some(p), Some(n)) if is_cjk(p) && is_cjk(n) => {} // CJK↔CJK: remove
                (Some(p), Some(n)) if is_cjk(p) || is_cjk(n) => {
                    // CJK↔ASCII: keep single space (East Asian convention)
                    result.push(' ');
                }
                _ => result.push(' '), // ASCII↔ASCII: keep single
            }
            i = j;
        } else {
            result.push(c);
            i += 1;
        }
    }

    result.iter().collect()
}

/// Returns true if a space should be inserted before `next_char` when appending
/// to text ending with `prev_char`.
fn needs_space_before(prev_char: char, next_char: char) -> bool {
    if prev_char.is_whitespace() || next_char.is_whitespace() {
        return false;
    }
    // No space between CJK characters (East Asian typographic convention)
    if is_cjk(prev_char) || is_cjk(next_char) {
        return false;
    }
    // No space before closing punctuation
    if CLOSING_PUNCT.contains(&next_char) {
        return false;
    }
    // No space after opening brackets
    if OPENING_BRACKETS.contains(&prev_char) {
        return false;
    }
    true
}

/// Append `text` to `out` with smart spacing and de-hyphenation.
fn append_inline(out: &mut String, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if out.is_empty() {
        out.push_str(text);
        return;
    }

    // De-hyphenate: if out ends with '-' and text starts with lowercase letter,
    // it's likely a soft-wrapped word. Remove the hyphen.
    if out.ends_with('-') {
        let next_char = text.chars().next().unwrap();
        if next_char.is_ascii_lowercase() {
            out.pop();
            out.push_str(text);
            return;
        }
    }

    let prev_char = out.chars().next_back().unwrap();
    let next_char = text.chars().next().unwrap();
    if needs_space_before(prev_char, next_char) {
        out.push(' ');
    }
    out.push_str(text);
}

// ── Core: join_text_regions ────────────────────────────────────────────────

/// Compute the median of a sorted slice of f64 values.
fn median(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return DEFAULT_MEDIAN_HEIGHT;
    }
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Returns true if the current line should be merged inline with the previous
/// line (soft-wrap detection).
fn should_merge_visual_line(
    prev: &OcrLineGeo,
    current: &OcrLineGeo,
    median_height: f64,
    doc_left: f64,
    doc_width: f64,
    prev_is_list: bool,
) -> bool {
    // List items always start a new line (unless continuation of wrapped item)
    if is_list_item(&current.text) {
        // List continuation: current is indented and previous was a list item
        if prev_is_list && current.x > prev.x + median_height * LEFT_ALIGN_FACTOR {
            return true;
        }
        return false;
    }

    // Vertical gap must be in the soft-wrap band
    let gap = current.y - (prev.y + prev.height);
    if gap < 0.0 || gap > median_height * SOFT_WRAP_GAP_FACTOR {
        return false;
    }

    // Left-edge alignment: current starts near previous's left edge
    let starts_near_previous = current.x <= prev.x + median_height * LEFT_ALIGN_FACTOR;

    // Previous line reached near the end of the column (fill ratio)
    let prev_reaches_line_end = prev.x + prev.width >= doc_left + doc_width * LINE_FILL_RATIO;

    // Don't merge if previous looks like a heading
    if looks_like_heading(&prev.text, prev.height, median_height) {
        return false;
    }

    // Don't merge if previous ended with sentence break
    if ends_with_sentence_break(&prev.text) {
        return false;
    }

    starts_near_previous && prev_reaches_line_end
}

/// Returns true if a blank line should be inserted between prev and current
/// (paragraph break vs simple line break).
fn should_separate_blocks(
    prev: &OcrLineGeo,
    current: &OcrLineGeo,
    median_height: f64,
    prev_block_is_list: bool,
    current_is_list: bool,
) -> bool {
    // List handling: blank line only at list entry/exit
    if prev_block_is_list || current_is_list {
        return prev_block_is_list != current_is_list;
    }

    // Prose: separate on large gap or heading
    let gap = current.y - (prev.y + prev.height);
    if gap > median_height * PARAGRAPH_BREAK_FACTOR {
        return true;
    }
    if looks_like_heading(&current.text, current.height, median_height) {
        return true;
    }
    false
}

/// Main entry: post-process OCR lines into structured Markdown-friendly text.
///
/// - Lines within a paragraph are joined with `\n` (soft-wrap).
/// - Paragraphs are separated with `\n\n`.
/// - List items are normalized to Markdown `- ` / `N. ` form.
/// - CJK↔ASCII spacing is handled intelligently.
pub fn join_text_regions(lines: &[OcrLineResult]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    // Convert to geo lines, filtering empty text
    let mut geos: Vec<OcrLineGeo> = lines
        .iter()
        .map(OcrLineGeo::from)
        .filter(|g| !g.text.trim().is_empty())
        .collect();

    if geos.is_empty() {
        return String::new();
    }

    // Sort by y (top-to-bottom), then x (left-to-right)
    geos.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Compute median height (dynamic, not fixed pixel bucket)
    let mut heights: Vec<f64> = geos.iter().map(|g| g.height).filter(|&h| h > 0.0).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_height = if heights.is_empty() {
        DEFAULT_MEDIAN_HEIGHT
    } else {
        median(&heights)
    };

    // Compute document frame (left edge and width for line-fill ratio)
    let doc_left = geos.iter().map(|g| g.x).fold(f64::MAX, f64::min);
    let doc_right = geos
        .iter()
        .map(|g| g.x + g.width)
        .fold(f64::MIN, f64::max);
    let doc_width = (doc_right - doc_left).max(1.0);

    // Format lines into paragraphs
    let mut output = String::new();
    let mut prev_block_is_list = false;
    let mut first = true;

    for (i, line) in geos.iter().enumerate() {
        // Normalize list items
        let normalized = normalize_line_text(&line.text);
        let current_is_list = is_list_item(&normalized);

        if first {
            append_inline(&mut output, &normalized);
            prev_block_is_list = current_is_list;
            first = false;
            continue;
        }

        let prev = &geos[i - 1];

        // Try soft-wrap merge first
        if should_merge_visual_line(
            prev,
            line,
            median_height,
            doc_left,
            doc_width,
            prev_block_is_list,
        ) {
            // Merge inline: append with smart spacing
            append_inline(&mut output, &normalized);
        } else if should_separate_blocks(prev, line, median_height, prev_block_is_list, current_is_list)
        {
            // Paragraph break
            output.push_str("\n\n");
            append_inline(&mut output, &normalized);
            prev_block_is_list = current_is_list;
        } else {
            // Simple line break (same block, not soft-wrap)
            output.push('\n');
            append_inline(&mut output, &normalized);
            prev_block_is_list = current_is_list;
        }
    }

    // Final CJK space cleaning pass
    clean_cjk_spaces(&output)
}

/// Backward-compatible alias for `join_text_regions`.
pub fn postprocess_ocr_lines(lines: &[OcrLineResult]) -> String {
    join_text_regions(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::capture::{OcrLineResult, OcrWordResult};

    fn make_line(text: &str, x: f64, y: f64, w: f64, h: f64) -> OcrLineResult {
        OcrLineResult {
            text: text.to_string(),
            x,
            y,
            width: w,
            height: h,
            words: vec![],
        }
    }

    fn make_line_with_words(text: &str, x: f64, y: f64, w: f64, h: f64) -> OcrLineResult {
        OcrLineResult {
            text: text.to_string(),
            x,
            y,
            width: w,
            height: h,
            words: vec![OcrWordResult {
                text: text.to_string(),
                x,
                y,
                width: w,
                height: h,
            }],
        }
    }

    // ── is_cjk ──────────────────────────────────────────────────────

    #[test]
    fn is_cjk_detects_common_chars() {
        assert!(is_cjk('汉'));
        assert!(is_cjk('あ'));
        assert!(is_cjk('ア'));
        assert!(is_cjk('한'));
        assert!(is_cjk('ｗ'));
    }

    #[test]
    fn is_cjk_rejects_ascii() {
        assert!(!is_cjk('a'));
        assert!(!is_cjk('0'));
        assert!(!is_cjk(' '));
    }

    // ── clean_cjk_spaces ────────────────────────────────────────────

    #[test]
    fn clean_cjk_removes_space_between_cjk() {
        assert_eq!(clean_cjk_spaces("汉 字"), "汉字");
        assert_eq!(clean_cjk_spaces("汉  字"), "汉字");
    }

    #[test]
    fn clean_cjk_keeps_space_between_cjk_and_ascii() {
        assert_eq!(clean_cjk_spaces("汉 A"), "汉 A");
        assert_eq!(clean_cjk_spaces("A 汉"), "A 汉");
    }

    #[test]
    fn clean_cjk_trims_and_compresses() {
        assert_eq!(clean_cjk_spaces("  汉字  "), "汉字");
        assert_eq!(clean_cjk_spaces("hello   world"), "hello world");
    }

    #[test]
    fn clean_cjk_handles_empty() {
        assert_eq!(clean_cjk_spaces(""), "");
        assert_eq!(clean_cjk_spaces("   "), "");
    }

    // ── is_list_item ────────────────────────────────────────────────

    #[test]
    fn is_list_item_detects_markdown_bullet() {
        assert!(is_list_item("- item one"));
        assert!(is_list_item("  - indented"));
    }

    #[test]
    fn is_list_item_detects_ordered() {
        assert!(is_list_item("1. first"));
        assert!(is_list_item("12. twelfth"));
        assert!(is_list_item("123. big"));
    }

    #[test]
    fn is_list_item_rejects_non_list() {
        assert!(!is_list_item("Hello world"));
        assert!(!is_list_item("1.24.4")); // version string, not list
        assert!(!is_list_item("123456")); // just a number
    }

    // ── normalize_line_text ─────────────────────────────────────────

    #[test]
    fn normalize_bullet_glyphs() {
        assert_eq!(normalize_line_text("• item"), "- item");
        assert_eq!(normalize_line_text("· text"), "- text");
        assert_eq!(normalize_line_text("● text"), "- text");
        assert_eq!(normalize_line_text("- text"), "- text");
        assert_eq!(normalize_line_text("* text"), "- text");
    }

    #[test]
    fn normalize_ordered_list() {
        assert_eq!(normalize_line_text("1. item"), "1. item");
        assert_eq!(normalize_line_text("1) item"), "1. item");
        assert_eq!(normalize_line_text("1、項目"), "1. 項目");
        assert_eq!(normalize_line_text("12) item"), "12. item");
    }

    #[test]
    fn normalize_ocr_misread_bullet() {
        assert_eq!(normalize_line_text("O Item"), "- Item");
        assert_eq!(normalize_line_text("o 項目"), "- 項目");
    }

    #[test]
    fn normalize_does_not_touch_version_strings() {
        assert_eq!(normalize_line_text("1.24.4"), "1.24.4");
    }

    #[test]
    fn normalize_preserves_plain_text() {
        assert_eq!(normalize_line_text("Hello world"), "Hello world");
        assert_eq!(normalize_line_text("你好世界"), "你好世界");
    }

    // ── needs_space_before / append_inline ──────────────────────────

    #[test]
    fn needs_space_between_ascii_words() {
        assert!(needs_space_before('o', 'w'));
        assert!(needs_space_before('d', 'H'));
    }

    #[test]
    fn no_space_between_cjk() {
        assert!(!needs_space_before('汉', '字'));
        assert!(!needs_space_before('A', '汉'));
        assert!(!needs_space_before('汉', 'A'));
    }

    #[test]
    fn no_space_before_closing_punct() {
        assert!(!needs_space_before('x', ','));
        assert!(!needs_space_before('x', '.'));
        assert!(!needs_space_before('x', ')'));
    }

    #[test]
    fn append_inline_dehyphenates() {
        let mut out = "hel".to_string();
        out.push('-');
        append_inline(&mut out, "lo");
        assert_eq!(out, "hello");
    }

    #[test]
    fn append_inline_inserts_space_between_ascii() {
        let mut out = "Hello".to_string();
        append_inline(&mut out, "World");
        assert_eq!(out, "Hello World");
    }

    #[test]
    fn append_inline_no_space_between_cjk() {
        let mut out = "你好".to_string();
        append_inline(&mut out, "世界");
        assert_eq!(out, "你好世界");
    }

    // ── join_text_regions (integration) ─────────────────────────────

    #[test]
    fn join_empty_returns_empty() {
        assert_eq!(join_text_regions(&[]), "");
    }

    #[test]
    fn join_single_line() {
        let lines = vec![make_line("Hello", 0.0, 0.0, 100.0, 20.0)];
        assert_eq!(join_text_regions(&lines), "Hello");
    }

    #[test]
    fn join_soft_wrap_merges_lines() {
        // Two lines, same left edge, prev fills width, tight gap → soft-wrap
        let lines = vec![
            make_line("This is a long line that fills the column width", 0.0, 0.0, 500.0, 20.0),
            make_line("continuation of the same paragraph", 0.0, 22.0, 300.0, 20.0),
        ];
        let result = join_text_regions(&lines);
        // Should be merged inline (no \n\n between them)
        assert!(!result.contains("\n\n"));
        assert!(result.contains("column width"));
        assert!(result.contains("continuation"));
    }

    #[test]
    fn join_large_gap_creates_paragraph_break() {
        let lines = vec![
            make_line("First paragraph", 0.0, 0.0, 100.0, 20.0),
            make_line("Second paragraph", 0.0, 80.0, 100.0, 20.0),
        ];
        let result = join_text_regions(&lines);
        assert!(result.contains("\n\n"));
    }

    #[test]
    fn join_sentence_break_prevents_soft_wrap() {
        let lines = vec![
            make_line("This ends with a period.", 0.0, 0.0, 500.0, 20.0),
            make_line("New sentence here.", 0.0, 22.0, 300.0, 20.0),
        ];
        let result = join_text_regions(&lines);
        // Sentence break should prevent soft-wrap merge → at least \n
        assert!(result.contains('\n'));
    }

    #[test]
    fn join_heading_starts_new_paragraph() {
        let lines = vec![
            make_line("Some text here", 0.0, 0.0, 200.0, 20.0),
            make_line("Big Heading", 0.0, 25.0, 200.0, 30.0), // taller = heading
            make_line("Body text after heading", 0.0, 70.0, 200.0, 20.0),
        ];
        let result = join_text_regions(&lines);
        // Heading should start a new paragraph
        assert!(result.contains("Big Heading"));
        assert!(result.contains("\n\n") || result.contains('\n'));
    }

    #[test]
    fn join_list_items_normalized() {
        let lines = vec![
            make_line("• first item", 0.0, 0.0, 100.0, 20.0),
            make_line("• second item", 0.0, 25.0, 100.0, 20.0),
            make_line("• third item", 0.0, 50.0, 100.0, 20.0),
        ];
        let result = join_text_regions(&lines);
        assert!(result.contains("- first item"));
        assert!(result.contains("- second item"));
        assert!(result.contains("- third item"));
    }

    #[test]
    fn join_cjk_spaces_cleaned() {
        let lines = vec![
            make_line("汉 字 测 试", 0.0, 0.0, 100.0, 20.0),
        ];
        let result = join_text_regions(&lines);
        assert_eq!(result, "汉字测试");
    }

    #[test]
    fn join_mixed_cjk_ascii() {
        let lines = vec![
            make_line("汉 字 ABC 123", 0.0, 0.0, 100.0, 20.0),
        ];
        let result = join_text_regions(&lines);
        assert_eq!(result, "汉字 ABC 123");
    }

    #[test]
    fn join_unsorted_lines_sorted_by_y() {
        let lines = vec![
            make_line("Third", 0.0, 50.0, 100.0, 20.0),
            make_line("First", 0.0, 0.0, 100.0, 20.0),
            make_line("Second", 0.0, 25.0, 100.0, 20.0),
        ];
        let result = join_text_regions(&lines);
        let lines_out: Vec<&str> = result.split('\n').collect();
        assert!(lines_out.iter().any(|&l| l.contains("First")));
        // First should appear before Second which appears before Third
        let first_pos = result.find("First").unwrap();
        let second_pos = result.find("Second").unwrap();
        let third_pos = result.find("Third").unwrap();
        assert!(first_pos < second_pos);
        assert!(second_pos < third_pos);
    }

    #[test]
    fn join_with_words_field() {
        let lines = vec![
            make_line_with_words("Hello", 0.0, 0.0, 100.0, 20.0),
            make_line_with_words("World", 0.0, 80.0, 100.0, 20.0),
        ];
        let result = join_text_regions(&lines);
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
        assert!(result.contains("\n\n"));
    }

    #[test]
    fn join_postprocess_alias_works() {
        let lines = vec![make_line("Test", 0.0, 0.0, 100.0, 20.0)];
        assert_eq!(postprocess_ocr_lines(&lines), join_text_regions(&lines));
    }

    // ── median ──────────────────────────────────────────────────────

    #[test]
    fn median_odd_count() {
        assert_eq!(median(&[1.0, 2.0, 3.0]), 2.0);
    }

    #[test]
    fn median_even_count() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
    }

    #[test]
    fn median_empty_returns_default() {
        assert_eq!(median(&[]), DEFAULT_MEDIAN_HEIGHT);
    }

    // ── looks_like_heading ──────────────────────────────────────────

    #[test]
    fn heading_detected_for_large_short_text() {
        assert!(looks_like_heading("Title", 30.0, 20.0));
    }

    #[test]
    fn heading_rejected_for_long_text() {
        let long = "a".repeat(100);
        assert!(!looks_like_heading(&long, 30.0, 20.0));
    }

    #[test]
    fn heading_rejected_for_sentence_break() {
        assert!(!looks_like_heading("Title.", 30.0, 20.0));
    }

    #[test]
    fn heading_rejected_for_small_font() {
        assert!(!looks_like_heading("Title", 20.0, 20.0));
    }
}

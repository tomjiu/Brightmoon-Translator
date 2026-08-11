//! P7: Font subsetting for PDF export.
//!
//! Reduces the size of embedded fonts by keeping only the glyphs used in the
//! translated text. A typical CJK font (msyh.ttf, ~17 MB) subsetted for a
//! single page of text drops to ~30–80 KB, which keeps bilingual PDF exports
//! small without bundling a font in the binary.
//!
//! ## Pipeline
//! 1. Parse the font's `cmap` table to map each character in the text to a
//!    glyph ID (GID).
//! 2. Use `subsetter::subset` to produce a new font containing only those
//!    GIDs (plus `.notdef`). The subsetter remaps GIDs to be consecutive
//!    starting at 1 and removes the `cmap` table — the result is a
//!    CID-keyed font suitable for PDF embedding.
//! 3. Return the subsetted font bytes along with the char→new-GID mapping
//!    so the PDF writer can address glyphs by CID.
//!
//! ## Fallback
//! If subsetting fails (malformed font, unsupported table layout, etc.),
//! callers should fall back to embedding the full font — the existing
//! `write_bilingual_pdf` path already does this.

use std::collections::BTreeMap;

/// Result of a successful subsetting operation.
pub struct SubsetResult {
    /// The subsetted font bytes (TTF/OTF with only the needed glyphs).
    pub font_bytes: Vec<u8>,
    /// Maps each Unicode character in the input text to its new glyph ID
    /// in the subsetted font. Use this as a GID→CID identity mapping when
    /// writing the PDF content stream.
    pub char_to_gid: BTreeMap<char, u16>,
}

/// Subset a font to only the glyphs needed to render `text`.
///
/// `font_data` is the raw bytes of a TTF, OTF, or TTC file. For TTC, the
/// first face is used (matching printpdf's behavior).
///
/// Returns `Err` if the font cannot be parsed or subsetting fails. Callers
/// should fall back to embedding the full font on error.
pub fn subset_font_for_text(font_data: &[u8], text: &str) -> Result<SubsetResult, String> {
    // Step 1: collect unique chars and map them to glyph IDs via cmap.
    let mut char_to_gid: BTreeMap<char, u16> = BTreeMap::new();
    let face = ttf_parser::Face::parse(font_data, 0)
        .map_err(|e| format!("font_subset: failed to parse font: {e}"))?;

    for ch in text.chars() {
        if char_to_gid.contains_key(&ch) {
            continue;
        }
        // tables().glyph_index returns Option<GlyphId>; 0 = .notdef.
        let gid = face.glyph_index(ch).map_or(0, |g| g.0);
        char_to_gid.insert(ch, gid);
    }

    // Always include .notdef (GID 0) — required by the OpenType spec.
    let mut gids: Vec<u16> = vec![0];
    for &gid in char_to_gid.values() {
        if gid != 0 {
            gids.push(gid);
        }
    }
    // Deduplicate while preserving ascending order (subsetter expects this).
    gids.sort_unstable();
    gids.dedup();

    // Step 2: build a GlyphRemapper and subset.
    let mut remapper = subsetter::GlyphRemapper::new();
    for &gid in &gids {
        remapper.remap(gid);
    }

    let sub = subsetter::subset(font_data, 0, &remapper)
        .map_err(|e| format!("font_subset: subsetter failed: {e:?}"))?;

    // Step 3: update char→GID mapping to use the remapped GIDs.
    let mut remapped = BTreeMap::new();
    for (ch, &old_gid) in &char_to_gid {
        let new_gid = if old_gid == 0 {
            // .notdef is always GID 0 in the subset.
            0
        } else {
            remapper.get(old_gid).unwrap_or(0)
        };
        remapped.insert(*ch, new_gid);
    }

    let original_kb = font_data.len() / 1024;
    let subset_kb = sub.len() / 1024;
    tracing::info!(
        "[P7] font subset: {} chars, {} glyphs, {} KB → {} KB ({:.1}% reduction)",
        char_to_gid.len(),
        gids.len(),
        original_kb,
        subset_kb,
        (1.0 - sub.len() as f64 / font_data.len() as f64) * 100.0
    );

    Ok(SubsetResult {
        font_bytes: sub,
        char_to_gid: remapped,
    })
}

/// Collect all unique characters from a slice of translated pages.
///
/// Includes source + translated text so the subset covers both halves of
/// the bilingual PDF. Also adds common ASCII punctuation / whitespace that
/// printpdf may reference for layout.
pub fn collect_text_chars(texts: &[&str]) -> String {
    use std::collections::BTreeSet;
    let mut set: BTreeSet<char> = BTreeSet::new();
    for t in texts {
        for ch in t.chars() {
            set.insert(ch);
        }
    }
    // Always include space, newline, and common ASCII punctuation so the
    // PDF writer's line-wrapping / spacing logic doesn't hit missing glyphs.
    for ch in [' ', '\n', '\r', '\t', '-', '.', ',', ':', ';', '!', '?', '(', ')', '"', '\''] {
        set.insert(ch);
    }
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_text_chars_dedupes_and_includes_ascii() {
        let s = collect_text_chars(&["hello", "world"]);
        // Should contain each letter once + space + ASCII punctuation.
        assert!(s.contains('h'));
        assert!(s.contains('w'));
        assert!(s.contains(' '));
        assert!(s.contains('.'));
        // No duplicates (length = unique chars + added ASCII).
        let unique: std::collections::BTreeSet<char> = s.chars().collect();
        assert_eq!(s.len(), unique.len());
    }

    #[test]
    fn collect_text_chars_handles_cjk() {
        let s = collect_text_chars(&["你好世界", "Hello"]);
        assert!(s.contains('你'));
        assert!(s.contains('H'));
        assert!(s.contains(' '));
    }

    #[test]
    fn subset_font_for_text_invalid_font_returns_err() {
        // Garbage bytes — should return Err, not panic.
        let result = subset_font_for_text(&[0x00, 0x01, 0x02, 0x03], "abc");
        assert!(result.is_err());
    }
}

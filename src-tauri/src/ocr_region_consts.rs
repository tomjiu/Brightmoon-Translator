//! Shared OCR region-frame geometry (CSS logical px).
//! Keep in sync with `src/components/ocrRegionGeometry.ts` (I2/I3).
//!
//! S5-7: cross-language consistency is enforced by `tests::consts_match_ts`
//! which parses the TS source and asserts both sides stay in lock-step.
//! If you change a value here, also change the TS file (and vice versa) —
//! the test will fail otherwise.

/// Toolbar height in CSS logical pixels.
pub const OCR_TOOLBAR_CSS_PX: f64 = 32.0;

/// Minimum frame width so full toolbar controls fit (CSS logical px).
pub const OCR_MIN_FRAME_CSS_W: f64 = 460.0;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Resolve the TS source path relative to this crate's manifest dir.
    fn ts_geometry_path() -> Option<PathBuf> {
        let manifest = env!("CARGO_MANIFEST_DIR");
        // src-tauri -> project root -> src/components/ocrRegionGeometry.ts
        let p = PathBuf::from(manifest)
            .join("..")
            .join("src")
            .join("components")
            .join("ocrRegionGeometry.ts");
        p.canonicalize().ok()
    }

    /// Extract the first `export const NAME = <number>;` value from TS source.
    fn extract_ts_const(ts: &str, name: &str) -> Option<f64> {
        let needle = format!("export const {name} = ");
        for line in ts.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(&needle) {
                let rest = &trimmed[needle.len()..];
                // take leading digits + optional decimal point
                let num_str: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                return num_str.parse::<f64>().ok();
            }
        }
        None
    }

    /// S5-7: assert Rust consts match the TS source-of-truth file.
    /// Skipped (not failed) when the TS file isn't reachable on disk
    /// (e.g. publishing the crate in isolation); in-repo CI always has it.
    #[test]
    fn consts_match_ts() {
        let Some(ts_path) = ts_geometry_path() else {
            eprintln!(
                "[ocr_region_consts] TS geometry file not found — skipping cross-language check"
            );
            return;
        };
        let ts = match std::fs::read_to_string(&ts_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "[ocr_region_consts] failed to read {}: {} — skipping cross-language check",
                    ts_path.display(),
                    e
                );
                return;
            }
        };

        let ts_toolbar = extract_ts_const(&ts, "OCR_TOOLBAR_HEIGHT_CSS")
            .expect("TS: OCR_TOOLBAR_HEIGHT_CSS missing or not a numeric literal");
        let ts_min_w = extract_ts_const(&ts, "OCR_MIN_FRAME_WIDTH_CSS")
            .expect("TS: OCR_MIN_FRAME_WIDTH_CSS missing or not a numeric literal");

        assert_eq!(
            OCR_TOOLBAR_CSS_PX, ts_toolbar,
            "OCR_TOOLBAR_CSS_PX (Rust={OCR_TOOLBAR_CSS_PX}) != OCR_TOOLBAR_HEIGHT_CSS (TS={ts_toolbar}). \
             Update both src-tauri/src/ocr_region_consts.rs and \
             src/components/ocrRegionGeometry.ts to the same value."
        );
        assert_eq!(
            OCR_MIN_FRAME_CSS_W, ts_min_w,
            "OCR_MIN_FRAME_CSS_W (Rust={OCR_MIN_FRAME_CSS_W}) != OCR_MIN_FRAME_WIDTH_CSS (TS={ts_min_w}). \
             Update both src-tauri/src/ocr_region_consts.rs and \
             src/components/ocrRegionGeometry.ts to the same value."
        );
    }

    /// Sanity: the extractor works on a known snippet.
    #[test]
    fn ts_const_extractor_unit() {
        let snippet = "export const FOO = 32;\nexport const BAR = 460.0;\nexport const BAZ = 'x';";
        assert_eq!(extract_ts_const(snippet, "FOO"), Some(32.0));
        assert_eq!(extract_ts_const(snippet, "BAR"), Some(460.0));
        assert_eq!(extract_ts_const(snippet, "BAZ"), None);
        assert_eq!(extract_ts_const(snippet, "MISSING"), None);
    }
}

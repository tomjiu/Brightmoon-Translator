//! Word under cursor via UIA ElementFromPoint, plus optional OCR near-cursor fallback.
//! Used by hover dictionary and OCR force pickup.

use super::SelectionBounds;
use crate::dictionary;
use crate::ocr_engine;
use std::time::{Duration, Instant};

/// Result of picking a word near the cursor.
#[derive(Debug, Clone)]
pub struct HoverPick {
    pub word: String,
    pub x: f64,
    pub y: f64,
    pub bounds: Option<SelectionBounds>,
    pub source: &'static str,
}

/// Extract a single word suitable for dictionary lookup from free text.
/// MTT-inspired unitization: prefer mid-token (under cursor feel), then first token,
/// then short CJK window — not the whole Name/Value dump.
pub fn extract_word_candidate(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    // Cap huge dumps (full document Name) before scanning
    let t = if t.chars().count() > 200 {
        let s: String = t.chars().take(200).collect();
        // keep owned for rest
        return extract_word_candidate_inner(&s);
    } else {
        t
    };
    extract_word_candidate_inner(t)
}

/// UI chrome / process names that UIA Name often returns instead of real text.
fn is_ui_chrome_word(w: &str) -> bool {
    let n = w.trim().to_ascii_lowercase();
    matches!(
        n.as_str(),
        "powershell"
            | "pwsh"
            | "cmd"
            | "command"
            | "prompt"
            | "windowsterminal"
            | "terminal"
            | "conhost"
            | "chrome"
            | "msedge"
            | "firefox"
            | "explorer"
            | "notepad"
            | "code"
            | "cursor"
            | "moontranslator"
            | "moon"
            | "translator"
            | "system"
            | "desktop"
            | "taskbar"
            | "start"
            | "search"
            | "settings"
            | "file"
            | "edit"
            | "view"
            | "help"
            | "ok"
            | "cancel"
            | "close"
            | "minimize"
            | "maximize"
    ) || n.ends_with(".exe")
        || n.contains("powershell")
        || n.contains("windows terminal")
}

fn extract_word_candidate_inner(t: &str) -> Option<String> {
    // Reject whole-window titles like "Administrator: Windows PowerShell"
    let lower = t.to_ascii_lowercase();
    if lower.contains("powershell")
        || lower.contains("windows terminal")
        || lower.contains("command prompt")
        || (t.contains(" - ") && t.chars().count() > 24)
    {
        // Still try to find a real token inside, but skip chrome tokens
    }

    if dictionary::is_single_word(t) && t.chars().count() <= 40 && !is_ui_chrome_word(t) {
        return Some(t.to_string());
    }

    let tokens: Vec<&str> = t
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-' || c == '\u{2019}'))
        .filter(|p| !p.is_empty())
        .collect();
    if !tokens.is_empty() {
        let mid = tokens[tokens.len() / 2];
        let mid = mid.trim_matches(|c: char| !c.is_alphanumeric());
        if dictionary::is_single_word(mid) && mid.chars().count() <= 40 && !is_ui_chrome_word(mid) {
            return Some(mid.to_string());
        }
        for part in tokens {
            let p = part.trim_matches(|c: char| !c.is_alphanumeric());
            if p.is_empty() || is_ui_chrome_word(p) {
                continue;
            }
            if dictionary::is_single_word(p) && p.chars().count() <= 40 {
                return Some(p.to_string());
            }
        }
    }

    let cjk_chars: Vec<char> = t
        .chars()
        .filter(|c| {
            matches!(
                c,
                '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '\u{f900}'..='\u{faff}'
            )
        })
        .collect();
    if !cjk_chars.is_empty() {
        let start = cjk_chars.len().saturating_sub(4) / 2;
        let cjk: String = cjk_chars.into_iter().skip(start).take(4).collect();
        if !cjk.is_empty() && dictionary::is_single_word(&cjk) {
            return Some(cjk);
        }
    }
    None
}

/// UIA ElementFromPoint → Name / Value / LegacyIAccessible → word candidate.
pub fn pick_word_at_cursor_uia() -> Option<HoverPick> {
    #[cfg(windows)]
    {
        pick_word_at_cursor_uia_win()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn pick_word_at_cursor_uia_win() -> Option<HoverPick> {
    use windows::core::Interface;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationValuePattern,
        UIA_ValuePatternId,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_err() {
            return None;
        }
        let cx = pt.x as f64;
        let cy = pt.y as f64;

        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL).ok()?;

        // ElementFromPoint is available on IUIAutomation (Win32 Accessibility).
        let element: IUIAutomationElement = match automation.ElementFromPoint(pt) {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!("[hover_pick] ElementFromPoint failed: {}", e);
                return None;
            },
        };

        let mut raw = String::new();
        if let Ok(name) = element.CurrentName() {
            let s = name.to_string();
            if !s.trim().is_empty() {
                raw = s;
            }
        }
        if raw.trim().is_empty() {
            if let Ok(pat) = element.GetCurrentPattern(UIA_ValuePatternId) {
                if let Ok(vp) = pat.cast::<IUIAutomationValuePattern>() {
                    if let Ok(v) = vp.CurrentValue() {
                        let s = v.to_string();
                        if !s.trim().is_empty() {
                            raw = s;
                        }
                    }
                }
            }
        }

        // Bounding rectangle if available
        let bounds = element.CurrentBoundingRectangle().ok().map(|r| {
            let w = (r.right - r.left).max(0) as f64;
            let h = (r.bottom - r.top).max(0) as f64;
            SelectionBounds {
                x: r.left as f64,
                y: r.top as f64,
                width: w,
                height: h,
            }
        });

        let word = extract_word_candidate(&raw)?;
        Some(HoverPick {
            word,
            x: cx,
            y: cy,
            bounds,
            source: "uia_point",
        })
    }
}

/// Wide horizontal strip around cursor (one text line) — less vertical chrome noise.
/// Default ~160×28 physical px (half 80×14). MTT-style: small target, not a square blob.
pub fn pick_word_near_cursor_ocr(half_w: i32, half_h: i32) -> Option<HoverPick> {
    #[cfg(windows)]
    {
        pick_word_near_cursor_ocr_win(half_w, half_h)
    }
    #[cfg(not(windows))]
    {
        let _ = (half_w, half_h);
        None
    }
}

/// Force long-rectangle OCR (for hover when UIA fails on image/browser).
pub fn pick_word_line_strip_ocr() -> Option<HoverPick> {
    // 180 wide × 28 tall (half 90 × 14)
    pick_word_near_cursor_ocr(90, 14)
}

#[cfg(windows)]
fn pick_word_near_cursor_ocr_win(half_w: i32, half_h: i32) -> Option<HoverPick> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut pt = POINT::default();
    unsafe {
        if GetCursorPos(&mut pt).is_err() {
            return None;
        }
    }
    // Prefer wide-and-short (text line). Allow tall only if caller asks.
    let half_w = half_w.clamp(48, 140);
    let half_h = half_h.clamp(10, 36);
    let left = pt.x - half_w;
    let top = pt.y - half_h;
    let width = (half_w * 2).max(1) as u32;
    let height = (half_h * 2).max(1) as u32;

    let img = crate::commands::capture::capture_area_gdi(left, top, width, height).ok()?;
    let png = {
        use image::codecs::png::{CompressionType, FilterType, PngEncoder};
        use image::ImageEncoder;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut buf = Vec::with_capacity((w as usize).saturating_mul(h as usize) / 2);
        let enc =
            PngEncoder::new_with_quality(&mut buf, CompressionType::Fast, FilterType::NoFilter);
        enc.write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
            .ok()?;
        buf
    };

    let text = ocr_engine::run_winrt_ocr(&png, None)
        .ok()
        .flatten()
        .filter(|t| !t.trim().is_empty())?;

    let word = extract_word_candidate(&text)?;
    if is_ui_chrome_word(&word) {
        return None;
    }
    Some(HoverPick {
        word,
        x: pt.x as f64,
        y: pt.y as f64,
        bounds: Some(SelectionBounds {
            x: left as f64,
            y: top as f64,
            width: width as f64,
            height: height as f64,
        }),
        source: "ocr_strip",
    })
}

/// Format dictionary results as short overlay body (word + phonetic + defs only).
/// Returns None when there is nothing useful to show (caller should MT or skip).
pub fn format_dict_body(
    word: &str,
    results: &[crate::models::dictionary::DictionaryResult],
) -> Option<String> {
    if !crate::dictionary::has_real_meanings(results) {
        return None;
    }
    let r0 = &results[0];
    let mut lines = Vec::new();
    // One headword line only (avoid duplicating "未找到" blocks)
    if let Some(ph) = &r0.phonetic {
        lines.push(format!("{}  {}", word, ph));
    } else {
        lines.push(word.to_string());
    }
    for m in r0.meanings.iter().take(6) {
        if m.part_of_speech.contains("未找到") {
            continue;
        }
        let defs: Vec<&str> = m
            .definitions
            .iter()
            .take(3)
            .map(|d| d.definition.as_str())
            .filter(|d| !d.is_empty() && !d.contains("未找到"))
            .collect();
        if defs.is_empty() {
            continue;
        }
        // Full definition lines (not truncated to one short phrase)
        for d in defs {
            let line = if m.part_of_speech.is_empty()
                || m.part_of_speech == "基本释义"
                || m.part_of_speech == "扩展释义"
            {
                d.to_string()
            } else {
                format!("[{}] {}", m.part_of_speech, d)
            };
            // Cap each def length for card, keep substance
            let line = if line.chars().count() > 120 {
                format!("{}…", line.chars().take(118).collect::<String>())
            } else {
                line
            };
            lines.push(line);
            if lines.len() >= 8 {
                break;
            }
        }
        if lines.len() >= 8 {
            break;
        }
    }
    if lines.len() <= 1 {
        return None;
    }
    Some(lines.join("\n"))
}

/// Simple rate limiter helper for hover (same word / same cell).
pub struct HoverDedupe {
    last_word: String,
    last_at: Instant,
    last_cell: (i32, i32),
}

impl HoverDedupe {
    pub fn new() -> Self {
        Self {
            last_word: String::new(),
            last_at: Instant::now() - Duration::from_secs(60),
            last_cell: (i32::MIN, i32::MIN),
        }
    }

    /// Cell size ~ 24px so small mouse jitter doesn't re-fire.
    pub fn should_skip(&mut self, word: &str, x: f64, y: f64, cooldown: Duration) -> bool {
        let cell = ((x / 24.0) as i32, (y / 24.0) as i32);
        if word == self.last_word && cell == self.last_cell && self.last_at.elapsed() < cooldown {
            return true;
        }
        self.last_word = word.to_string();
        self.last_cell = cell;
        self.last_at = Instant::now();
        false
    }
}

impl Default for HoverDedupe {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_english_token() {
        assert_eq!(
            extract_word_candidate("  hello  ").as_deref(),
            Some("hello")
        );
        assert_eq!(
            extract_word_candidate("Say hello, world!").as_deref(),
            Some("Say")
        );
    }

    #[test]
    fn extract_cjk_phrase() {
        let w = extract_word_candidate("你好世界测试").unwrap();
        assert!(w.chars().count() <= 4);
        assert!(!w.is_empty());
    }
}

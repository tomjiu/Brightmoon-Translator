//! Word under cursor via UIA `ElementFromPoint`, plus optional OCR near-cursor fallback.
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
    extract_word_candidate_with_hint(text, None)
}

/// Prefer the token under an estimated horizontal position inside the control.
/// `cursor_ratio` is 0.0..=1.0 across the element width (from left).
pub fn extract_word_candidate_with_hint(text: &str, cursor_ratio: Option<f64>) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    // Cap huge dumps (full document Name) before scanning
    let owned: String;
    let t = if t.chars().count() > 200 {
        owned = t.chars().take(200).collect();
        owned.as_str()
    } else {
        t
    };
    if let Some(r) = cursor_ratio {
        if let Some(w) = extract_word_at_ratio(t, r.clamp(0.0, 1.0)) {
            return Some(w);
        }
    }
    extract_word_candidate_inner(t)
}

/// Map horizontal ratio → char index → surrounding alphanumeric / CJK token.
fn extract_word_at_ratio(t: &str, ratio: f64) -> Option<String> {
    let chars: Vec<char> = t.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let idx = ((chars.len() as f64 - 1.0) * ratio).round() as usize;
    let idx = idx.min(chars.len() - 1);

    // Expand to Latin/digit word or short CJK window around idx
    let c = chars[idx];
    if c.is_alphanumeric() || c == '\'' || c == '-' || c == '\u{2019}' {
        let mut start = idx;
        while start > 0 {
            let prev = chars[start - 1];
            if prev.is_alphanumeric() || prev == '\'' || prev == '-' || prev == '\u{2019}' {
                start -= 1;
            } else {
                break;
            }
        }
        let mut end = idx + 1;
        while end < chars.len() {
            let next = chars[end];
            if next.is_alphanumeric() || next == '\'' || next == '-' || next == '\u{2019}' {
                end += 1;
            } else {
                break;
            }
        }
        let word: String = chars[start..end].iter().collect();
        let word = word.trim_matches(|ch: char| !ch.is_alphanumeric());
        if !word.is_empty()
            && word.chars().count() <= 40
            && dictionary::is_single_word(word)
            && !is_ui_chrome_word(word)
        {
            return Some(word.to_string());
        }
    }

    if matches!(
        c,
        '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '\u{f900}'..='\u{faff}'
    ) {
        let start = idx.saturating_sub(1);
        let end = (idx + 3).min(chars.len());
        let cjk: String = chars[start..end]
            .iter()
            .copied()
            .filter(|ch| {
                matches!(
                    ch,
                    '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '\u{f900}'..='\u{faff}'
                )
            })
            .take(4)
            .collect();
        if !cjk.is_empty() && dictionary::is_single_word(&cjk) {
            return Some(cjk);
        }
    }
    None
}

/// UI chrome / process names that UIA Name often returns instead of real text.
pub fn is_ui_chrome_word(w: &str) -> bool {
    let t = w.trim();
    if t.is_empty() {
        return true;
    }
    let n = t.to_ascii_lowercase();
    // Window titles: "App - Document", "Administrator: Windows PowerShell"
    // P1: colon check only for chrome tokens (no spaces, short) —
    // "Warning: ..." / "Note: ..." are real sentences, not chrome.
    if (t.contains(" - ") || t.contains(" — ")) && t.chars().count() > 12 {
        return true;
    }
    if t.contains(':') && !t.contains(' ') && t.chars().count() > 12 && t.chars().count() < 40 {
        return true;
    }
    if n.ends_with(".exe") || n.contains(".exe ") {
        return true;
    }
    matches!(
        n.as_str(),
        "powershell"
            | "pwsh"
            | "cmd"
            | "command"
            | "prompt"
            | "windowsterminal"
            | "windows terminal"
            | "terminal"
            | "conhost"
            | "chrome"
            | "google chrome"
            | "msedge"
            | "microsoft edge"
            | "firefox"
            | "explorer"
            | "file explorer"
            | "notepad"
            | "code"
            | "visual studio code"
            | "cursor"
            | "moontranslator"
            | "moon translator"
            | "moon"
            | "translator"
            | "system"
            | "desktop"
            | "taskbar"
            | "start"
            | "search"
            | "settings"
            | "windows"
            | "administrator"
            | "file"
            | "edit"
            | "view"
            | "help"
            | "ok"
            | "cancel"
            | "close"
            | "minimize"
            | "maximize"
            | "application"
            | "window"
            | "document"
            | "untitled"
            | "新标签页"
            | "新标签"
            | "空白页"
    ) || n.contains("powershell")
        || n.contains("windows terminal")
        || n.contains("visual studio")
        || n.starts_with("microsoft ")
        || n.contains("任务管理器")
        || n.contains("设置") && t.chars().count() <= 6
}

/// True if candidate looks like a process/app product name (reject for hover).
pub fn looks_like_app_or_process_name(w: &str) -> bool {
    if is_ui_chrome_word(w) {
        return true;
    }
    let t = w.trim();
    let lower = t.to_ascii_lowercase();
    if lower.ends_with(".exe")
        || lower.ends_with(".dll")
        || lower.ends_with(".app")
        || lower.contains("powershell")
        || lower == "cmd"
        || lower == "pwsh"
        || lower == "bash"
        || lower == "zsh"
    {
        return true;
    }
    // CamelCase product ids (WinStore, TextInput) — not normal dictionary words
    if t.chars().count() >= 8
        && !t.contains(' ')
        && t.chars().all(|c| c.is_ascii_alphanumeric())
        && t.chars().any(|c| c.is_ascii_uppercase())
        && t.chars().any(|c| c.is_ascii_lowercase())
        && t.chars().filter(char::is_ascii_uppercase).count() >= 2
        && t.chars().next().is_some_and(|c| c.is_ascii_uppercase())
    {
        return true;
    }
    false
}

/// True if `s` is a run of > 4 CJK characters (a phrase/sentence, not a short headword).
fn is_long_cjk_run(s: &str) -> bool {
    let cjk_count = s
        .chars()
        .filter(|c| {
            matches!(
                c,
                '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '\u{f900}'..='\u{faff}'
            )
        })
        .count();
    cjk_count > 4 && cjk_count * 2 >= s.chars().count()
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

    // Whole-token fast path: a single Latin/digit word, or a short CJK phrase
    // (≤ 4 chars). Longer CJK runs must go through the short-window logic below so
    // we return a dictionary headword window instead of a whole sentence.
    let cjk_total: usize = t
        .chars()
        .filter(|c| {
            matches!(
                c,
                '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '\u{f900}'..='\u{faff}'
            )
        })
        .count();
    let is_short_cjk = cjk_total > 0 && cjk_total <= 4;
    if dictionary::is_single_word(t)
        && t.chars().count() <= 40
        && !is_ui_chrome_word(t)
        && (cjk_total == 0 || is_short_cjk)
    {
        return Some(t.to_string());
    }

    let tokens: Vec<&str> = t
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-' || c == '\u{2019}'))
        .filter(|p| !p.is_empty())
        .collect();
    if !tokens.is_empty() {
        let mid = tokens[tokens.len() / 2];
        let mid = mid.trim_matches(|c: char| !c.is_alphanumeric());
        if dictionary::is_single_word(mid)
            && mid.chars().count() <= 40
            && !is_ui_chrome_word(mid)
            && !is_long_cjk_run(mid)
        {
            return Some(mid.to_string());
        }
        for part in tokens {
            let p = part.trim_matches(|c: char| !c.is_alphanumeric());
            if p.is_empty() || is_ui_chrome_word(p) || is_long_cjk_run(p) {
                continue;
            }
            if dictionary::is_single_word(p) && p.chars().count() <= 40 {
                return Some(p.to_string());
            }
        }
    }

    // CJK: prefer 2-char then 1 then 3 near mid (dictionary headwords), not random 4-char dump
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
        let mid = cjk_chars.len() / 2;
        for len in [2usize, 1, 3, 4] {
            if cjk_chars.len() < len {
                continue;
            }
            let start = mid.saturating_sub(len / 2).min(cjk_chars.len() - len);
            let cjk: String = cjk_chars[start..start + len].iter().collect();
            if !cjk.is_empty() && !is_ui_chrome_word(&cjk) && dictionary::is_single_word(&cjk) {
                return Some(cjk);
            }
        }
    }
    None
}

/// MTT: prefer sentence around cursor ratio (punctuation / newline bounds).
pub fn extract_sentence_candidate_with_hint(
    text: &str,
    cursor_ratio: Option<f64>,
) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let owned: String;
    let t = if t.chars().count() > 400 {
        owned = t.chars().take(400).collect();
        owned.as_str()
    } else {
        t
    };
    let chars: Vec<char> = t.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let idx = if let Some(r) = cursor_ratio {
        ((chars.len() as f64 - 1.0) * r.clamp(0.0, 1.0)).round() as usize
    } else {
        chars.len() / 2
    }
    .min(chars.len() - 1);

    let is_hard_bound = |c: char| matches!(c, '!' | '?' | '。' | '！' | '？' | '\n' | '\r' | '…');
    // `.` / `;` only if not mid-abbreviation / decimal
    let is_dot_bound = |i: usize| -> bool {
        let c = chars[i];
        if c == '…' {
            return true;
        }
        if c != '.' && c != ';' && c != '；' {
            return false;
        }
        if c == '.' {
            // decimal: digit.digit
            let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
            let next_digit = i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
            if prev_digit && next_digit {
                return false;
            }
            // ellipsis ...
            if i + 2 < chars.len() && chars[i + 1] == '.' && chars[i + 2] == '.' {
                return true;
            }
            // common English abbreviations (Mr. Dr. etc.) — letter before dot, letter after space+letter
            if i > 0 && chars[i - 1].is_ascii_alphabetic() {
                let ab: String = chars[i.saturating_sub(3)..i]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase();
                if ab.ends_with("mr")
                    || ab.ends_with("ms")
                    || ab.ends_with("dr")
                    || ab.ends_with("st")
                    || ab.ends_with("jr")
                    || ab.ends_with("sr")
                    || ab.ends_with("vs")
                    || ab == "etc"
                    || ab.ends_with("e.g")
                    || ab.ends_with("i.e")
                {
                    return false;
                }
            }
        }
        true
    };
    let is_bound_at = |i: usize| is_hard_bound(chars[i]) || is_dot_bound(i);

    let mut start = idx;
    while start > 0 && !is_bound_at(start - 1) {
        start -= 1;
    }
    // skip leading whitespace after previous bound
    while start < chars.len() && chars[start].is_whitespace() {
        start += 1;
    }
    let mut end = idx.max(start) + 1;
    end = end.min(chars.len());
    while end < chars.len() && !is_bound_at(end) {
        end += 1;
    }
    // include trailing sentence punct / ellipsis
    if end < chars.len() && is_bound_at(end) {
        if chars[end] == '.'
            && end + 2 < chars.len()
            && chars[end + 1] == '.'
            && chars[end + 2] == '.'
        {
            end += 3;
        } else {
            end += 1;
        }
    }
    let s: String = chars[start..end.min(chars.len())].iter().collect();
    let s = s.trim();
    let n = s.chars().count();
    if !(2..=160).contains(&n) {
        return None;
    }
    if is_ui_chrome_word(s) {
        return None;
    }
    // Prefer not returning a lone title-case chrome token as "sentence"
    if n <= 4 && !s.chars().any(char::is_whitespace) && dictionary::is_single_word(s) {
        // still ok for short CJK / short English clause without space
    }
    Some(s.to_string())
}

/// True when focused control looks like an editable field (MTT: hide hover while typing).
pub fn is_editable_control_focused() -> bool {
    #[cfg(windows)]
    {
        is_editable_control_focused_win()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
fn is_editable_control_focused_win() -> bool {
    use windows::core::Interface;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationValuePattern, UIA_ComboBoxControlTypeId,
        UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_ValuePatternId,
    };

    // SAFETY: COM init + UIA calls on this thread. All COM objects returned
    // are reference-counted and released automatically by the `windows` crate.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let automation: IUIAutomation = match CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) {
            Ok(a) => a,
            Err(_) => return false,
        };
        let focused = match automation.GetFocusedElement() {
            Ok(e) => e,
            Err(_) => return false,
        };
        if let Ok(ct) = focused.CurrentControlType() {
            let id = ct.0;
            // P1 fix: Document controls (PDF viewer, Word, browser doc) should
            // allow hover — only block truly editable Edit/ComboBox. Document
            // was wrongly blocked, making hover dead on PDF/Word/browser pages.
            if id == UIA_EditControlTypeId.0 || id == UIA_ComboBoxControlTypeId.0 {
                return true;
            }
            if id == UIA_DocumentControlTypeId.0 {
                // Only block when the doc is editable (ValuePattern not read-only);
                // a read-only document is a reading surface → allow hover.
                if let Ok(pat) = focused.GetCurrentPattern(UIA_ValuePatternId) {
                    if let Ok(vp) = pat.cast::<IUIAutomationValuePattern>() {
                        if let Ok(ro) = vp.CurrentIsReadOnly() {
                            return !ro.as_bool();
                        }
                    }
                }
                return false; // readonly or unknown → allow hover
            }
        }
        if let Ok(pat) = focused.GetCurrentPattern(UIA_ValuePatternId) {
            if let Ok(vp) = pat.cast::<IUIAutomationValuePattern>() {
                if let Ok(ro) = vp.CurrentIsReadOnly() {
                    if !ro.as_bool() {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// UIA `ElementFromPoint` → Name / Value → word or sentence candidate.
/// `sentence`: true → sentence unit (MTT container-ish); false → word.
pub fn pick_word_at_cursor_uia() -> Option<HoverPick> {
    pick_at_cursor_uia(false)
}

pub fn pick_at_cursor_uia(sentence: bool) -> Option<HoverPick> {
    #[cfg(windows)]
    {
        pick_at_cursor_uia_win(sentence)
    }
    #[cfg(not(windows))]
    {
        let _ = sentence;
        None
    }
}

#[cfg(windows)]
fn pick_at_cursor_uia_win(sentence: bool) -> Option<HoverPick> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement};

    // S1-6: use shared cursor reader instead of a local GetCursorPos FFI block.
    let (px, py) = crate::win::cursor_pos_raw()?;
    let pt = POINT { x: px, y: py };
    let cx = f64::from(pt.x);
    let cy = f64::from(pt.y);

    // SAFETY: COM init + UIA calls on this thread. All COM objects are
    // reference-counted; pt is a valid screen POINT from cursor_pos_raw.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL).ok()?;

        let element: IUIAutomationElement = match automation.ElementFromPoint(pt) {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!("[hover_pick] ElementFromPoint failed: {}", e);
                return None;
            },
        };

        // Reject pure chrome controls (title bar, tab strip) — never parse Name as page text.
        if element_is_chrome_only(&element) {
            let ct = element.CurrentControlType().ok().map(|c| c.0);
            tracing::info!(
                "[hover_pick] reject: chrome-only element control_type={:?}",
                ct
            );
            return None;
        }

        // 1) TextPattern at point = real document text (non-invasive). Prefer this only.
        if let Some(pick) = try_text_pattern_at_point(&element, pt, cx, cy, sentence) {
            return Some(pick);
        }
        let ct = element.CurrentControlType().ok().map(|c| c.0);
        tracing::info!(
            "[hover_pick] TextPattern miss → ValuePattern fallback control_type={:?}",
            ct
        );

        // 2) Writable ValuePattern (edit fields) — content, not window title.
        //    Do NOT use CurrentName: it is Accessibility name (app title, "Google", "PowerShell").
        let raw = value_pattern_text_if_editable(&element)?;
        if raw.trim().is_empty() {
            return None;
        }

        let bounds = element.CurrentBoundingRectangle().ok().map(|r| {
            let w = f64::from((r.right - r.left).max(0));
            let h = f64::from((r.bottom - r.top).max(0));
            SelectionBounds {
                x: f64::from(r.left),
                y: f64::from(r.top),
                width: w,
                height: h,
            }
        });

        let ratio = bounds.as_ref().and_then(|b| {
            if b.width > 2.0 {
                Some(((cx - b.x) / b.width).clamp(0.0, 1.0))
            } else {
                None
            }
        });
        let word = if sentence {
            extract_sentence_candidate_with_hint(&raw, ratio)
                .or_else(|| extract_word_candidate_with_hint(&raw, ratio))?
        } else {
            extract_word_candidate_with_hint(&raw, ratio)?
        };
        if is_ui_chrome_word(&word) {
            return None;
        }
        Some(HoverPick {
            word,
            x: cx,
            y: cy,
            bounds,
            source: if sentence {
                "uia_value_sentence"
            } else {
                "uia_value"
            },
        })
    }
}

/// Window / pane / title-bar-ish controls: no free-hover text (would only yield app names).
#[cfg(windows)]
fn element_is_chrome_only(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::Win32::UI::Accessibility::{
        UIA_GroupControlTypeId, UIA_MenuBarControlTypeId, UIA_MenuControlTypeId,
        UIA_PaneControlTypeId, UIA_TabControlTypeId, UIA_TabItemControlTypeId,
        UIA_TitleBarControlTypeId, UIA_ToolBarControlTypeId, UIA_WindowControlTypeId,
    };
    // SAFETY: Read-only property access on a borrowed IUIAutomationElement.
    // The COM pointer is valid for the lifetime of `element`.
    unsafe {
        if let Ok(ct) = element.CurrentControlType() {
            let id = ct.0;
            if id == UIA_WindowControlTypeId.0
                || id == UIA_TitleBarControlTypeId.0
                || id == UIA_MenuBarControlTypeId.0
                || id == UIA_MenuControlTypeId.0
                || id == UIA_ToolBarControlTypeId.0
                || id == UIA_TabControlTypeId.0
                || id == UIA_TabItemControlTypeId.0
                || id == UIA_PaneControlTypeId.0
                || id == UIA_GroupControlTypeId.0
            {
                // Pane/Group often wrap real text — only skip if no TextPattern and short Name
                if id == UIA_PaneControlTypeId.0 || id == UIA_GroupControlTypeId.0 {
                    return false;
                }
                return true;
            }
        }
        false
    }
}

#[cfg(windows)]
fn value_pattern_text_if_editable(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Option<String> {
    use windows::core::Interface;
    use windows::Win32::UI::Accessibility::{
        IUIAutomationValuePattern, UIA_DocumentControlTypeId, UIA_EditControlTypeId,
        UIA_ValuePatternId,
    };
    // SAFETY: Read-only UIA property/pattern queries on a borrowed element.
    // COM pointers from GetCurrentPattern are reference-counted.
    unsafe {
        let is_edit = element
            .CurrentControlType()
            .ok()
            .is_some_and(|ct| ct.0 == UIA_EditControlTypeId.0 || ct.0 == UIA_DocumentControlTypeId.0);
        let pat = element.GetCurrentPattern(UIA_ValuePatternId).ok()?;
        let vp: IUIAutomationValuePattern = pat.cast().ok()?;
        let ro = vp
            .CurrentIsReadOnly()
            .ok()
            .is_none_or(windows::Win32::Foundation::BOOL::as_bool);
        // Only trust Value when editable edit/document, or non-readonly with substantial text
        let v = vp.CurrentValue().ok()?.to_string();
        let v = v.trim().to_string();
        if v.is_empty() {
            return None;
        }
        if is_edit && !ro {
            return Some(v);
        }
        // Avoid treating read-only "Google" / status strings as page words
        if ro || v.chars().count() < 2 || is_ui_chrome_word(&v) {
            return None;
        }
        if is_edit {
            return Some(v);
        }
        None
    }
}

/// TextPattern.RangeFromPoint + ExpandToEnclosingUnit(Word|Sentence).
#[cfg(windows)]
fn try_text_pattern_at_point(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    pt: windows::Win32::Foundation::POINT,
    cx: f64,
    cy: f64,
    sentence: bool,
) -> Option<HoverPick> {
    use windows::core::Interface;
    use windows::Win32::UI::Accessibility::{
        IUIAutomationTextPattern, TextUnit_Paragraph, TextUnit_Word, UIA_TextPatternId,
    };

    // SAFETY: Read-only UIA pattern/range queries on a borrowed element.
    // COM pointers are reference-counted; pt was obtained from cursor_pos_raw.
    unsafe {
        let Ok(pat) = element.GetCurrentPattern(UIA_TextPatternId) else {
            tracing::info!(
                "[hover_pick] no TextPattern on element (control_type={:?})",
                element.CurrentControlType().ok().map(|c| c.0)
            );
            return None;
        };
        let text_pattern: IUIAutomationTextPattern = pat.cast().ok()?;
        let Ok(range) = text_pattern.RangeFromPoint(pt) else {
            tracing::info!("[hover_pick] RangeFromPoint failed");
            return None;
        };
        // UIA has no Sentence unit — expand Paragraph then trim to one sentence.
        let unit = if sentence {
            TextUnit_Paragraph
        } else {
            TextUnit_Word
        };
        let _ = range.ExpandToEnclosingUnit(unit);
        let max_chars = if sentence { 400 } else { 80 };
        let text = range.GetText(max_chars).ok()?.to_string();
        let raw = text.trim();
        if raw.is_empty() || is_ui_chrome_word(raw) {
            return None;
        }
        // Estimate cursor ratio from element bounds when available.
        let cursor_ratio = element.CurrentBoundingRectangle().ok().map(|r| {
            let w = f64::from((r.right - r.left).max(1));
            ((cx - f64::from(r.left)) / w).clamp(0.0, 1.0)
        });
        let word = if sentence {
            extract_sentence_candidate_with_hint(raw, cursor_ratio.or(Some(0.45)))
                .unwrap_or_else(|| raw.chars().take(160).collect::<String>())
        } else {
            // MTT: don't blindly trust UIA's TextUnit_Word expansion — some controls return
            // a phrase/line for the Word unit. Extract the word under the cursor ratio with
            // the same algorithm as the ValuePattern path, so word-boundary handling is
            // consistent across both UIA text sources. extract_word_candidate_with_hint
            // already applies is_single_word / is_ui_chrome_word / length guards.
            match extract_word_candidate_with_hint(raw, cursor_ratio) {
                Some(w) if w.chars().count() <= 40 => w,
                other => {
                    tracing::info!(
                        "[hover_pick] word extraction failed raw={:?} ratio={:?} got={:?} (control_type={:?})",
                        raw.chars().take(100).collect::<String>(),
                        cursor_ratio,
                        other,
                        element.CurrentControlType().ok().map(|c| c.0)
                    );
                    return None;
                },
            }
        };
        let word = word.trim();
        if word.is_empty() || word.chars().count() > 160 || is_ui_chrome_word(word) {
            return None;
        }
        if looks_like_app_or_process_name(word) {
            return None;
        }
        // P0: place the card near the *word*, not the whole element — the
        // element's bounding rect (a paragraph / full control) put cards far
        // from the hovered token. The expanded range's own rect is the token.
        let bounds = super::uiautomation::text_range_bounds(&range).or_else(|| {
            element
                .CurrentBoundingRectangle()
                .ok()
                .map(|r| SelectionBounds {
                    x: f64::from(r.left),
                    y: f64::from(r.top),
                    width: f64::from((r.right - r.left).max(0)),
                    height: f64::from((r.bottom - r.top).max(0)),
                })
        });
        Some(HoverPick {
            word: word.to_string(),
            x: cx,
            y: cy,
            bounds,
            source: if sentence {
                "uia_textpattern_sentence"
            } else {
                "uia_textpattern_word"
            },
        })
    }
}

/// Wide horizontal strip around cursor (one text line) — less vertical chrome noise.
/// Default ~160×28 physical px (half 80×14). MTT-style: small target, not a square blob.
/// Skips pure chrome under cursor; slightly larger strip when UIA looks image-like.
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
    // 180 wide × 28 tall (half 90 × 14); image-like may enlarge inside OCR path
    pick_word_near_cursor_ocr(90, 14)
}

/// UIA under cursor looks like image content (Image, or Pane/Group without `TextPattern`).
/// Editable `TextPattern` already preferred on the UIA path; this only sizes OCR strip.
#[cfg(windows)]
fn element_is_image_like(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::core::Interface;
    use windows::Win32::UI::Accessibility::{
        IUIAutomationTextPattern, UIA_GroupControlTypeId, UIA_ImageControlTypeId,
        UIA_PaneControlTypeId, UIA_TextPatternId,
    };
    // SAFETY: Read-only UIA property/pattern queries on a borrowed element.
    // COM pointers from GetCurrentPattern are reference-counted.
    unsafe {
        // TextPattern present → not image-like for OCR sizing (text path preferred)
        if let Ok(pat) = element.GetCurrentPattern(UIA_TextPatternId) {
            if pat.cast::<IUIAutomationTextPattern>().is_ok() {
                return false;
            }
        }
        if let Ok(ct) = element.CurrentControlType() {
            let id = ct.0;
            if id == UIA_ImageControlTypeId.0 {
                return true;
            }
            // Pane/Group without TextPattern often host bitmaps/canvas text
            if id == UIA_PaneControlTypeId.0 || id == UIA_GroupControlTypeId.0 {
                return true;
            }
        }
        false
    }
}

/// Pure chrome for OCR: title bar / menus / tabs / window — do not strip-OCR these.
/// Pane/Group are not blocked here (may be image-like content).
#[cfg(windows)]
fn element_is_ocr_chrome(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::Win32::UI::Accessibility::{
        UIA_MenuBarControlTypeId, UIA_MenuControlTypeId, UIA_TabControlTypeId,
        UIA_TabItemControlTypeId, UIA_TitleBarControlTypeId, UIA_ToolBarControlTypeId,
        UIA_WindowControlTypeId,
    };
    // SAFETY: Read-only CurrentControlType query on a borrowed element.
    // The COM pointer is valid for the lifetime of `element`.
    unsafe {
        if let Ok(ct) = element.CurrentControlType() {
            let id = ct.0;
            return id == UIA_WindowControlTypeId.0
                || id == UIA_TitleBarControlTypeId.0
                || id == UIA_MenuBarControlTypeId.0
                || id == UIA_MenuControlTypeId.0
                || id == UIA_ToolBarControlTypeId.0
                || id == UIA_TabControlTypeId.0
                || id == UIA_TabItemControlTypeId.0;
        }
        false
    }
}

#[cfg(windows)]
fn uia_element_at_cursor() -> Option<(
    windows::Win32::Foundation::POINT,
    windows::Win32::UI::Accessibility::IUIAutomationElement,
)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement};

    // S1-6: use shared cursor reader instead of a local GetCursorPos FFI block.
    let (px, py) = crate::win::cursor_pos_raw()?;
    let pt = POINT { x: px, y: py };

    // SAFETY: COM init + UIA ElementFromPoint on a valid screen POINT.
    // Returned COM objects are reference-counted.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL).ok()?;
        let element: IUIAutomationElement = automation.ElementFromPoint(pt).ok()?;
        Some((pt, element))
    }
}

#[cfg(windows)]
fn pick_word_near_cursor_ocr_win(half_w: i32, half_h: i32) -> Option<HoverPick> {
    use windows::Win32::Foundation::POINT;

    let mut half_w = half_w;
    let mut half_h = half_h;
    let pt;

    // Probe UIA under cursor: skip pure chrome; enlarge strip for image-like controls.
    if let Some((cursor_pt, element)) = uia_element_at_cursor() {
        pt = cursor_pt;
        if element_is_ocr_chrome(&element) {
            tracing::trace!("[hover_pick] OCR strip skipped: chrome control under cursor");
            return None;
        }
        if element_is_image_like(&element) {
            // Slightly larger strip over images/canvas (still line-ish, not a square blob)
            half_w = half_w.max(110);
            half_h = half_h.max(22);
            tracing::trace!(
                "[hover_pick] OCR strip image-like → larger {}x{}",
                half_w * 2,
                half_h * 2
            );
        }
    } else {
        // S1-6: fall back to shared cursor reader (UIA path failed entirely).
        let (x, y) = crate::win::cursor_pos_raw()?;
        pt = POINT { x, y };
    }

    // Prefer wide-and-short (text line). Allow tall only if caller asks / image-like bump.
    let half_w = half_w.clamp(48, 160);
    let half_h = half_h.clamp(10, 48);
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
        x: f64::from(pt.x),
        y: f64::from(pt.y),
        bounds: Some(SelectionBounds {
            x: f64::from(left),
            y: f64::from(top),
            width: f64::from(width),
            height: f64::from(height),
        }),
        source: "ocr_strip",
    })
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
            last_at: Instant::now().checked_sub(Duration::from_mins(1)).unwrap(),
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
        // Mid-token preference (closer to "under cursor" than first token)
        assert_eq!(
            extract_word_candidate("Say hello, world!").as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn rejects_ui_chrome_and_titles() {
        assert!(extract_word_candidate("PowerShell").is_none());
        assert!(extract_word_candidate("Windows PowerShell").is_none());
        assert!(extract_word_candidate("Administrator: Windows PowerShell").is_none());
        assert!(is_ui_chrome_word("pwsh"));
        assert!(!is_ui_chrome_word("translate"));
    }

    #[test]
    fn rejects_app_or_process_like_names() {
        assert!(looks_like_app_or_process_name("pwsh"));
        assert!(looks_like_app_or_process_name("notepad.exe"));
        assert!(looks_like_app_or_process_name("WinStore"));
        assert!(!looks_like_app_or_process_name("translate"));
        assert!(!looks_like_app_or_process_name("hello"));
    }

    #[test]
    fn extract_cjk_phrase() {
        let w = extract_word_candidate("你好世界测试").unwrap();
        assert!(w.chars().count() <= 4);
        assert!(!w.is_empty());
    }

    #[test]
    fn extract_word_at_cursor_ratio_prefers_under_point() {
        // Left side → first token; right side → last token
        assert_eq!(
            extract_word_candidate_with_hint("alpha beta gamma", Some(0.05)).as_deref(),
            Some("alpha")
        );
        assert_eq!(
            extract_word_candidate_with_hint("alpha beta gamma", Some(0.95)).as_deref(),
            Some("gamma")
        );
        assert_eq!(
            extract_word_candidate_with_hint("one two three", Some(0.5)).as_deref(),
            Some("two")
        );
    }

    #[test]
    fn extract_sentence_around_ratio() {
        let s = extract_sentence_candidate_with_hint(
            "Hello world. Second sentence here! Third.",
            Some(0.55),
        )
        .unwrap();
        assert!(s.to_ascii_lowercase().contains("second"));
        assert!(!s.contains("Third"));
    }

    #[test]
    fn extract_sentence_keeps_abbrev_and_decimal() {
        let s = extract_sentence_candidate_with_hint(
            "See Dr. Smith at 3.14 pm. Next line starts here.",
            Some(0.2),
        )
        .unwrap();
        assert!(s.contains("Dr. Smith") || s.contains("3.14"));
        assert!(!s.contains("Next line"));
    }
}

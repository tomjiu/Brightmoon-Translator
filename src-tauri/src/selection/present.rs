//! Unified selection/hover presentation router (QTranslate D vs Q + Easydict pop).
//! Word → dictionary card only; sentence/phrase → MT card; junk → reject (never MT).

use super::hover_pick::{is_ui_chrome_word, looks_like_app_or_process_name};
use crate::dictionary;
use crate::models::dictionary::{Definition, DictionaryResult, Meaning};
use crate::overlay;
use tauri::{AppHandle, Manager};

/// Coarse text class for display routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextClass {
    Word,
    Phrase,
    Sentence,
    Junk,
}

/// One POS block on a dictionary card.
#[derive(Debug, Clone)]
pub struct DictMeaning {
    pub pos: String,
    pub defs: Vec<String>,
}

/// Structured dictionary card (no pure-text re-parse for HTML).
#[derive(Debug, Clone)]
pub struct DictCard {
    pub word: String,
    pub phonetic: Option<String>,
    pub meanings: Vec<DictMeaning>,
}

impl DictCard {
    pub fn from_results(word: &str, results: &[DictionaryResult]) -> Option<Self> {
        if !dictionary::has_real_meanings(results) {
            return None;
        }
        let r0 = &results[0];
        let mut meanings = Vec::new();
        for m in r0.meanings.iter().take(6) {
            if m.part_of_speech.contains("未找到") {
                continue;
            }
            let defs: Vec<String> = m
                .definitions
                .iter()
                .take(3)
                .map(|d| d.definition.trim().to_string())
                .filter(|d| !d.is_empty() && !d.contains("未找到"))
                .map(|d| {
                    if d.chars().count() > 120 {
                        format!("{}…", d.chars().take(118).collect::<String>())
                    } else {
                        d
                    }
                })
                .collect();
            if defs.is_empty() {
                continue;
            }
            let pos = if m.part_of_speech == "基本释义" || m.part_of_speech == "扩展释义" {
                String::new()
            } else {
                m.part_of_speech.clone()
            };
            meanings.push(DictMeaning { pos, defs });
        }
        if meanings.is_empty() {
            return None;
        }
        let head = if r0.word.trim().is_empty() {
            word.trim().to_string()
        } else {
            r0.word.trim().to_string()
        };
        Some(DictCard {
            word: head,
            phonetic: r0.phonetic.clone().filter(|p| !p.trim().is_empty()),
            meanings,
        })
    }

    /// Legacy plain-text body for callers that still format lines.
    pub fn to_body_text(&self) -> String {
        let mut lines = Vec::new();
        if let Some(ph) = &self.phonetic {
            lines.push(format!("{}  {}", self.word, ph));
        } else {
            lines.push(self.word.clone());
        }
        for m in &self.meanings {
            for d in &m.defs {
                if m.pos.is_empty() {
                    lines.push(d.clone());
                } else {
                    lines.push(format!("[{}] {}", m.pos, d));
                }
                if lines.len() >= 8 {
                    break;
                }
            }
            if lines.len() >= 8 {
                break;
            }
        }
        lines.join("\n")
    }
}

/// Classify selection text for routing. Junk never goes to MT.
pub fn classify_text(text: &str) -> TextClass {
    let t = text.trim();
    if t.is_empty() {
        return TextClass::Junk;
    }
    if !t.chars().any(|c| c.is_alphanumeric() || is_cjk_char(c)) {
        return TextClass::Junk;
    }
    if is_ui_chrome_word(t) || looks_like_app_or_process_name(t) {
        return TextClass::Junk;
    }
    // Multi-line / long → sentence
    if t.contains('\n') || t.chars().count() > 64 {
        return TextClass::Sentence;
    }
    let words = t.split_whitespace().filter(|w| !w.is_empty()).count();
    if words >= 4 {
        return TextClass::Sentence;
    }
    if words >= 2 {
        if t.contains(['.', '!', '?', '。', '！', '？', ';', '；']) {
            return TextClass::Sentence;
        }
        return TextClass::Phrase;
    }
    // Single token
    if t.chars().all(|c| c.is_ascii_digit()) {
        return TextClass::Junk;
    }
    if dictionary::is_single_word(t) && t.chars().count() <= 32 {
        return TextClass::Word;
    }
    TextClass::Sentence
}

fn is_cjk_char(c: char) -> bool {
    matches!(
        c,
        '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '\u{f900}'..='\u{faff}'
    )
}

fn hover_dict_source(app: &AppHandle) -> String {
    app.try_state::<crate::AppState>()
        .map(|s| {
            s.system
                .config
                .blocking_lock()
                .selection_ux
                .hover_dict_source
                .clone()
        })
        .unwrap_or_else(|| "auto".into())
        .to_ascii_lowercase()
}

/// ECDICT → DictionaryResult (shared by hover + selection word path).
async fn lookup_ecdict(
    word: &str,
    pool: &sqlx::SqlitePool,
) -> Result<Vec<DictionaryResult>, String> {
    use sqlx::Row;
    let key = word.trim().to_lowercase();
    let row = match sqlx::query(
        "SELECT word, phonetic, definition, translation, pos FROM stardict WHERE word = ?1 COLLATE NOCASE LIMIT 1",
    )
    .bind(&key)
    .fetch_optional(pool)
    .await
    {
        Ok(r) => r,
        Err(_) => sqlx::query(
            "SELECT word, phonetic, definition, translation FROM stardict WHERE word = ?1 COLLATE NOCASE LIMIT 1",
        )
        .bind(&key)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?,
    };
    let Some(row) = row else {
        return Ok(vec![]);
    };
    let head: String = row.try_get("word").unwrap_or_else(|_| word.to_string());
    let phonetic: Option<String> = row.try_get("phonetic").ok().flatten();
    let translation: Option<String> = row.try_get("translation").ok().flatten();
    let definition: Option<String> = row.try_get("definition").ok().flatten();
    let pos_raw: Option<String> = row.try_get("pos").ok().flatten();
    let default_pos = pos_raw
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();
    let mut meanings: Vec<Meaning> = Vec::new();
    if let Some(tr) = translation {
        for line in tr.split(['\n', '\\']) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let (pos, def) = if let Some((p, rest)) = line.split_once('.') {
                let p = p.trim();
                if p.len() <= 6 && p.chars().all(|c| c.is_ascii_alphabetic()) {
                    (p, rest.trim())
                } else {
                    (default_pos.as_str(), line)
                }
            } else {
                (default_pos.as_str(), line)
            };
            push_ecdict_def(&mut meanings, pos, def);
        }
    }
    if meanings.is_empty() {
        if let Some(def) = definition {
            for line in def.split('\n').take(4) {
                push_ecdict_def(&mut meanings, default_pos.as_str(), line);
            }
        }
    }
    if meanings.is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![DictionaryResult {
        word: head,
        phonetic: phonetic.filter(|p| !p.is_empty()).map(|p| {
            if p.starts_with('/') || p.starts_with('[') {
                p
            } else {
                format!("/{}/", p)
            }
        }),
        meanings,
        source_urls: vec![],
    }])
}

fn push_ecdict_def(meanings: &mut Vec<Meaning>, pos: &str, def_text: &str) {
    let def_text = def_text.trim();
    if def_text.is_empty() {
        return;
    }
    let pos_key = if pos.is_empty() { "" } else { pos };
    if let Some(m) = meanings.iter_mut().find(|m| m.part_of_speech == pos_key) {
        if m.definitions.len() < 6 {
            m.definitions.push(Definition {
                definition: def_text.to_string(),
                example: None,
                synonyms: vec![],
                antonyms: vec![],
            });
        }
        return;
    }
    if meanings.len() >= 6 {
        return;
    }
    meanings.push(Meaning {
        part_of_speech: pos_key.to_string(),
        definitions: vec![Definition {
            definition: def_text.to_string(),
            example: None,
            synonyms: vec![],
            antonyms: vec![],
        }],
    });
}

/// ECDICT → Youdao (same pool for hover and selection words).
pub async fn lookup_word(app: &AppHandle, text: &str) -> Option<DictCard> {
    let word = text.trim();
    if word.is_empty() || !dictionary::is_single_word(word) {
        return None;
    }
    let source = hover_dict_source(app);
    let use_ecdict = matches!(source.as_str(), "auto" | "ecdict" | "local");
    let use_youdao = matches!(source.as_str(), "auto" | "youdao" | "online");

    let mut results = Vec::new();
    let mut hit = "miss";

    if use_ecdict && !dictionary::is_cjk(word) {
        if let Some(state) = app.try_state::<crate::AppState>() {
            if let Some(pool) = state.ecdict_pool.as_ref() {
                if let Ok(body) = lookup_ecdict(word, pool).await {
                    if dictionary::has_real_meanings(&body) {
                        results = body;
                        hit = "ecdict";
                    }
                }
            }
        }
    }

    if results.is_empty() && use_youdao {
        let dict = dictionary::Dictionary::new();
        results = if dictionary::is_cjk(word) {
            let mut found = Vec::new();
            let chars: Vec<char> = word.chars().collect();
            if chars.len() > 1 {
                for len in (1..=chars.len().min(4)).rev() {
                    for start in 0..=(chars.len() - len) {
                        let sub: String = chars[start..start + len].iter().collect();
                        if let Ok(r) = dict.lookup_chinese(&sub).await {
                            if dictionary::has_real_meanings(&r) {
                                found = r;
                                break;
                            }
                        }
                    }
                    if !found.is_empty() {
                        break;
                    }
                }
            }
            if found.is_empty() {
                dict.lookup_chinese(word).await.unwrap_or_default()
            } else {
                found
            }
        } else {
            dict.lookup(word).await.unwrap_or_default()
        };
        if dictionary::has_real_meanings(&results) {
            hit = "youdao";
        }
    }

    let card = DictCard::from_results(word, &results);
    tracing::info!(
        "[hover] word={:?} hit={} card={}",
        word.chars().take(32).collect::<String>(),
        hit,
        card.is_some()
    );
    card
}

fn cursor_pos() -> (f64, f64) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut pt = POINT::default();
        if unsafe { GetCursorPos(&mut pt).is_ok() } {
            return (pt.x as f64, pt.y as f64);
        }
    }
    (100.0, 100.0)
}

fn dict_card_size(card: &DictCard) -> (f64, f64) {
    let body = card.to_body_text();
    let line_n = body.lines().count().max(1) as f64;
    let h = (64.0 + line_n * 22.0).clamp(80.0, 300.0);
    let longest = body
        .lines()
        .map(|l| {
            l.chars()
                .map(|c| if c >= '\u{3000}' { 15.0_f64 } else { 8.0 })
                .sum::<f64>()
        })
        .fold(0.0_f64, f64::max)
        .max(96.0);
    let w = (longest + 56.0).clamp(220.0, 460.0);
    (w, h)
}

/// Show structured dictionary card (badge + phonetic + POS).
pub async fn present_dict_card(app: &AppHandle, card: &DictCard, pos: Option<(f64, f64)>) {
    let (cx, cy) = pos.unwrap_or_else(cursor_pos);
    let place = overlay::OverlayPosition::at_cursor(cx, cy);
    let (w, h) = dict_card_size(card);
    let dismiss = app
        .try_state::<crate::AppState>()
        .map(|s| {
            s.system
                .config
                .blocking_lock()
                .overlay_auto_dismiss_ms
                .max(3500)
        })
        .unwrap_or(4500);
    let html = overlay::html_builder::build_dict_card_structured(card, dismiss);
    let _ = overlay::window_manager::create_overlay_window_ex(
        app, &html, place.x, place.y, w, h, true, false,
    );
}

/// Machine-translate card (source + translation).
pub async fn present_mt_card(app: &AppHandle, source: &str, pos: Option<(f64, f64)>) {
    let Some(state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let (from, to, dismiss) = {
        let c = state.system.config.lock().await;
        (
            c.default_from.clone(),
            c.default_to.clone(),
            c.overlay_auto_dismiss_ms,
        )
    };
    let source = source.trim();
    if source.is_empty() {
        return;
    }

    match state
        .translation
        .service
        .run_full(
            crate::models::translation::TranslateChannel::Selection,
            source,
            &from,
            &to,
        )
        .await
    {
        Ok(resp) => {
            let display = {
                let joined = resp.display_text();
                if joined.is_empty() {
                    format!("（无翻译结果）\n{}", source)
                } else {
                    joined
                }
            };
            let (cx, cy) = pos.unwrap_or_else(cursor_pos);
            let place = overlay::OverlayPosition::at_cursor(cx, cy);
            let (w, h) = overlay::window_manager::estimate_mt_card_size(&display);
            let content = overlay::OverlayContent {
                source: source.to_string(),
                translated: display,
                source_app: Some("selection".into()),
                window_title: None,
            };
            let html = overlay::html_builder::build_html(
                &content,
                overlay::OverlayLevel::Standard,
                dismiss.max(5000),
                None,
            );
            let _ = overlay::window_manager::create_overlay_window_ex(
                app, &html, place.x, place.y, w, h, true, false,
            );
        },
        Err(e) => {
            tracing::warn!("[present] mt failed: {e}");
            let (cx, cy) = pos.unwrap_or_else(cursor_pos);
            let place = overlay::OverlayPosition::at_cursor(cx, cy);
            let content = overlay::OverlayContent {
                source: source.to_string(),
                translated: format!("翻译失败：{e}"),
                source_app: Some("selection".into()),
                window_title: None,
            };
            let html =
                overlay::html_builder::build_html(&content, overlay::OverlayLevel::Minimal, 4000, None);
            let _ = overlay::window_manager::create_overlay_window_ex(
                app, &html, place.x, place.y, 320.0, 120.0, true, false,
            );
        },
    }
}

/// Hover path: dictionary only; miss / junk → silent (never MT).
pub async fn present_hover_dictionary(app: &AppHandle, word: &str, x: f64, y: f64) {
    let word = word.trim();
    if word.is_empty() {
        return;
    }
    #[cfg(windows)]
    if crate::selection::mouse_hook::key_pressed_within_ms(400) {
        return;
    }
    match classify_text(word) {
        TextClass::Junk => {
            tracing::info!(
                "[hover] word={:?} source=hover route=reject",
                word.chars().take(24).collect::<String>()
            );
            return;
        },
        TextClass::Sentence | TextClass::Phrase if !dictionary::is_single_word(word) => {
            // Sentence hover is routed by caller to present_selection; defensive.
            present_selection(app, word).await;
            return;
        },
        _ => {},
    }
    let Some(card) = lookup_word(app, word).await else {
        return;
    };
    #[cfg(windows)]
    if crate::selection::mouse_hook::key_pressed_within_ms(400) {
        return;
    }
    present_dict_card(app, &card, Some((x, y))).await;
}

/// Unified selection / pop / hotkey path.
/// Word → dict card (miss silent, no MT junk); phrase/sentence → MT; junk → reject.
pub async fn present_selection(app: &AppHandle, text: &str) {
    let trimmed = text.trim();
    let preview: String = trimmed.chars().take(40).collect();
    let class = classify_text(trimmed);
    let route = match class {
        TextClass::Word => "dict",
        TextClass::Phrase => "mt",
        TextClass::Sentence => "mt",
        TextClass::Junk => "reject",
    };
    tracing::info!(
        "[pop] pending_len={} preview={:?} route={}",
        trimmed.chars().count(),
        preview,
        route
    );

    match class {
        TextClass::Junk => {},
        TextClass::Word => {
            if let Some(card) = lookup_word(app, trimmed).await {
                present_dict_card(app, &card, None).await;
            } else {
                tracing::info!("[present] word dict miss — no MT");
            }
        },
        TextClass::Phrase | TextClass::Sentence => {
            // Short CJK/English phrase that is still is_single_word → try dict first
            if dictionary::is_single_word(trimmed) && trimmed.chars().count() <= 32 {
                if let Some(card) = lookup_word(app, trimmed).await {
                    present_dict_card(app, &card, None).await;
                    return;
                }
            }
            present_mt_card(app, trimmed, None).await;
        },
    }
}

/// Whether text is safe to show pop button (not chrome/junk).
pub fn accept_for_pop(text: &str) -> bool {
    !matches!(classify_text(text), TextClass::Junk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_word_sentence_junk() {
        assert_eq!(classify_text("hello"), TextClass::Word);
        assert_eq!(
            classify_text("This is a full sentence."),
            TextClass::Sentence
        );
        assert_eq!(classify_text("PowerShell"), TextClass::Junk);
        assert_eq!(classify_text("pwsh"), TextClass::Junk);
        assert_eq!(classify_text(""), TextClass::Junk);
        assert_eq!(classify_text("foo bar"), TextClass::Phrase);
    }

    #[test]
    fn dict_card_from_empty_none() {
        assert!(DictCard::from_results("hello", &[]).is_none());
    }
}

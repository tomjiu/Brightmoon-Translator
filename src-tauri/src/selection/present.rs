//! Unified selection/hover presentation router (`QTranslate` D vs Q + Easydict pop).
//! Word → dictionary card only; sentence/phrase → MT card; junk → reject (never MT).

use super::hover_pick::{is_ui_chrome_word, looks_like_app_or_process_name};
use crate::commands::dictionary_cmd::{
    ComprehensiveEntry, PhoneticInfo,
};
use crate::dictionary;
use crate::models::dictionary::{Definition, DictionaryResult, Meaning};
use crate::models::translation::TranslateResponse;
use crate::overlay;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use std::time::Duration;

/// Coarse text class for display routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextClass {
    Word,
    Phrase,
    Sentence,
    Junk,
}

/// One POS block on a dictionary card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictMeaning {
    pub pos: String,
    pub defs: Vec<String>,
}

/// Bilingual example shown under a word (en + zh pair).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictExample {
    pub en: String,
    pub zh: String,
}

/// One Collins entry block (英英释义 + 双语例句).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictCollins {
    pub pos: String,
    pub pos_cn: String,
    pub english_def: String,
    pub examples: Vec<DictExample>,
}

/// Structured dictionary card (no pure-text re-parse for HTML).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictCard {
    pub word: String,
    pub phonetic: Option<String>,
    pub meanings: Vec<DictMeaning>,
    pub phonetics: Vec<PhoneticInfo>,
    pub audio_url: Option<String>,
    pub examples: Vec<DictExample>,
    pub collins: Vec<DictCollins>,
    pub sources: Vec<String>,
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
            phonetics: vec![],
            audio_url: None,
            examples: vec![],
            collins: vec![],
            sources: vec![],
        })
    }

    /// Build a rich card from the full multi-source `ComprehensiveEntry`
    /// (ECDICT + 有道 + DictionaryAPI.dev + Collins + Oxford + GPT).
    pub fn from_comprehensive(entry: &ComprehensiveEntry) -> Option<Self> {
        if entry.sources.is_empty() {
            return None;
        }
        let word = entry.word.trim().to_string();
        let phonetic = entry
            .phonetics
            .first()
            .and_then(|p| p.text.as_ref())
            .map(|t| {
                let t = t.trim();
                if t.starts_with('/') || t.starts_with('[') {
                    t.to_string()
                } else {
                    format!("/{t}/")
                }
            })
            .filter(|p| !p.is_empty());

        // Chinese translation first (most useful at a glance).
        let mut meanings = Vec::new();
        if let Some(zh) = entry
            .chinese_translation
            .as_ref()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty() && !t.contains("未找到"))
        {
            let defs: Vec<String> = zh
                .split(['；', ';'])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .take(6)
                .collect();
            if !defs.is_empty() {
                meanings.push(DictMeaning {
                    pos: String::new(),
                    defs,
                });
            }
        }

        // English definitions from DictionaryAPI.dev (POS blocks).
        for om in entry.online_meanings.iter().take(4) {
            let defs: Vec<String> = om
                .definitions
                .iter()
                .take(3)
                .map(|d| d.definition.trim().to_string())
                .filter(|d| !d.is_empty())
                .collect();
            if defs.is_empty() {
                continue;
            }
            let pos = om.part_of_speech.trim().to_string();
            if let Some(m) = meanings.iter_mut().find(|m| m.pos == pos) {
                m.defs.extend(defs);
            } else {
                meanings.push(DictMeaning { pos, defs });
            }
        }

        // ECDICT English definitions as a last-resort fallback block.
        if meanings.is_empty() {
            let defs: Vec<String> = entry
                .english_definitions
                .iter()
                .take(6)
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty())
                .collect();
            if !defs.is_empty() {
                meanings.push(DictMeaning {
                    pos: String::new(),
                    defs,
                });
            }
        }
        if meanings.is_empty() {
            return None;
        }

        let examples: Vec<DictExample> = entry
            .examples
            .iter()
            .take(3)
            .map(|e| DictExample {
                en: e.en.clone(),
                zh: e.zh.clone(),
            })
            .collect();

        let collins: Vec<DictCollins> = entry
            .collins_entries
            .iter()
            .take(2)
            .map(|c| DictCollins {
                pos: c.pos.clone(),
                pos_cn: c.pos_cn.clone(),
                english_def: c.english_def.clone(),
                examples: c
                    .examples
                    .iter()
                    .take(1)
                    .map(|e| DictExample {
                        en: e.en.clone(),
                        zh: e.zh.clone(),
                    })
                    .collect(),
            })
            .collect();

        Some(DictCard {
            word,
            phonetic,
            meanings,
            phonetics: entry.phonetics.clone(),
            audio_url: entry.audio_url.clone(),
            examples,
            collins,
            sources: entry.sources.clone(),
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
        if !self.examples.is_empty() {
            lines.push(String::new());
            for e in self.examples.iter().take(2) {
                lines.push(e.en.clone());
                if !e.zh.is_empty() {
                    lines.push(e.zh.clone());
                }
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

async fn hover_dict_source(app: &AppHandle) -> String {
    let source = if let Some(s) = app.try_state::<crate::AppState>() {
        // tokio Mutex: await (never blocking_lock — that panics when called
        // from inside the async runtime, see PR9 dev log).
        s.system.config.lock().await.selection_ux.hover_dict_source.clone()
    } else {
        "auto".into()
    };
    source.to_ascii_lowercase()
}

/// ECDICT → `DictionaryResult` (shared by hover + selection word path).
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
                format!("/{p}/")
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
    let source = hover_dict_source(app).await;
    let use_ecdict = matches!(source.as_str(), "auto" | "ecdict" | "local");
    let use_youdao = matches!(source.as_str(), "auto" | "youdao" | "online");

    // EN words: prefer the full multi-source implementation (ECDICT local +
    // 有道/DictionaryAPI/Collins/Oxford/GPT) so the card shows 音标/音频/例句/柯林斯.
    // Bounded with a timeout so hover stays snappy even when the network is slow.
    if use_youdao && !dictionary::is_cjk(word) {
        if let Some(state) = app.try_state::<crate::AppState>() {
            match tokio::time::timeout(
                Duration::from_millis(1500),
                crate::commands::dictionary_cmd::lookup_word_multi_source(word.to_string(), state),
            )
            .await
            {
                Ok(Ok(entry)) => {
                    if let Some(card) = DictCard::from_comprehensive(&entry) {
                        tracing::info!(
                            "[hover] word={:?} hit=multi card=true",
                            word.chars().take(32).collect::<String>()
                        );
                        return Some(card);
                    }
                    tracing::debug!("[hover] multi-source had no meanings for {:?}", word);
                },
                Ok(Err(e)) => tracing::debug!("[hover] multi-source miss for {:?}: {}", word, e),
                Err(_) => tracing::debug!("[hover] multi-source timeout for {:?}", word),
            }
        }
    }

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
        if unsafe { GetCursorPos(&raw mut pt).is_ok() } {
            return (f64::from(pt.x), f64::from(pt.y));
        }
    }
    (100.0, 100.0)
}

/// Rendered line count for `text` inside a content area `content_w` px wide.
/// Latin ~8px/char, CJK ~15px/char (matches the width estimate below).
fn rendered_lines(text: &str, content_w: f64) -> usize {
    let px: f64 = text
        .chars()
        .map(|c| if c >= '\u{3000}' { 15.0_f64 } else { 8.0 })
        .sum();
    ((px / content_w.max(1.0)).ceil() as usize).max(1)
}

fn dict_card_size(card: &DictCard) -> (f64, f64) {
    // Width first: longest text → card width (clamped).
    let longest = card
        .meanings
        .iter()
        .flat_map(|m| m.defs.iter())
        .chain(card.examples.iter().map(|e| &e.en))
        .chain(card.collins.iter().map(|c| &c.english_def))
        .chain(card.phonetic.iter())
        .map(|l| {
            l.chars()
                .map(|c| if c >= '\u{3000}' { 15.0_f64 } else { 8.0 })
                .sum::<f64>()
        })
        .fold(card.word.chars().count() as f64 * 9.0, f64::max)
        .max(120.0);
    let w = (longest + 56.0).clamp(240.0, 480.0);
    let content_w = w - 26.0; // 13px card padding each side

    // Height: wrapping-aware per block (matches render_dict_card_shell CSS).
    //   padding 22px + badge ~21px + head ~26px
    //   def 13px*1.5lh + 4px mt ≈ 24px/rendered line
    //   section header ≈ 31px; example en/zh ≈ 19.5px each line + 5px mt
    //   collins ≈ 19.5px/line + 5px mt; sources ≈ 20px
    // Deliberately generous: an underestimate clips the translation (the whole
    // point of the card), and the JS fit script only ever grows the window.
    let mut h = 22.0 + 21.0 + 26.0;
    let mut total_defs = 0usize;
    for m in &card.meanings {
        for d in &m.defs {
            if total_defs >= 8 {
                break;
            }
            // POS badge sits inline before the def (~30px), reducing wrap width.
            let badge = if m.pos.is_empty() { 0.0 } else { 30.0 };
            let lines = rendered_lines(d, (content_w - badge).max(60.0));
            h += (lines as f64 * 24.0).max(26.0);
            total_defs += 1;
        }
        if total_defs >= 8 {
            break;
        }
    }
    if !card.examples.is_empty() {
        h += 31.0; // section header
        for e in card.examples.iter().take(3) {
            if e.en.trim().is_empty() {
                continue;
            }
            let en_l = rendered_lines(&e.en, content_w);
            let zh_l = if e.zh.trim().is_empty() {
                0
            } else {
                rendered_lines(&e.zh, content_w)
            };
            h += (en_l + zh_l).max(1) as f64 * 21.5 + 5.0;
        }
    }
    if !card.collins.is_empty() {
        h += 31.0;
        for c in card.collins.iter().take(2) {
            let lines = rendered_lines(&c.english_def, content_w - 30.0);
            h += lines as f64 * 21.5 + 5.0;
        }
    }
    if !card.sources.is_empty() {
        h += 22.0;
    }
    let h = h.clamp(110.0, 720.0);
    (w, h)
}

/// Show structured dictionary card (badge + phonetic + POS).
/// `bounds`: when present, the card is placed near the word (below, or above
/// when the word is near the screen bottom) instead of at the cursor, so it
/// doesn't occlude the word being hovered (Fix 16).
/// `steal_focus`: false for hover/dict (never steals focus); true for
/// user-initiated lookups.
pub async fn present_dict_card(
    app: &AppHandle,
    card: &DictCard,
    pos: Option<(f64, f64)>,
    bounds: Option<&crate::selection::SelectionBounds>,
    steal_focus: bool,
) {
    let (w, h) = dict_card_size(card);
    let place = if let Some(b) = bounds {
        let (cx, cy) = pos.unwrap_or_else(cursor_pos);
        let (x, y) = overlay::positioner::place_near_bounds(b, w, h, cx, cy);
        overlay::OverlayPosition::new(x, y, w, h)
    } else {
        let (cx, cy) = pos.unwrap_or_else(cursor_pos);
        overlay::OverlayPosition::at_cursor(cx, cy)
    };
    let payload = overlay::translate_card::TranslateCardData::Dict(
        overlay::translate_card::DictCardData { card: card.clone() },
    );
    // No-focus cards placed near bounds (hover) keep the hovered word alive so
    // parking on it doesn't dismiss the card. Focus cards close on blur instead.
    let keep_alive = if steal_focus {
        None
    } else {
        bounds.cloned()
    };
    if let Err(e) = overlay::translate_card::show_translate_card(
        app,
        &payload,
        place.x,
        place.y,
        w,
        h,
        overlay::translate_card::TranslateCardOptions {
            steal_focus,
            keep_alive,
        },
    )
    .await
    {
        tracing::warn!("[present] dict card failed: {e}");
    }
}

/// Machine-translate card (source + translation).
/// `bounds`: when present, the card is placed below the selection instead of
/// at the cursor — fixes cards appearing at the (moved) cursor after the
/// 1-2s translate delay.
/// `steal_focus`: false for hover (dict miss fallback); true for
/// user-initiated selection/pop.
pub async fn present_mt_card(
    app: &AppHandle,
    source: &str,
    pos: Option<(f64, f64)>,
    bounds: Option<&crate::selection::SelectionBounds>,
    steal_focus: bool,
) {
    let Some(state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let (from, to) = {
        let c = state.system.config.lock().await;
        (c.default_from.clone(), c.default_to.clone())
    };
    let source = source.trim();
    if source.is_empty() {
        return;
    }
    let opts = overlay::translate_card::TranslateCardOptions {
        steal_focus,
        // No-focus (hover) MT cards placed near the hovered word keep the word
        // alive: parking the cursor on it must not dismiss the card. Focus
        // cards close on blur instead.
        keep_alive: if steal_focus { None } else { bounds.cloned() },
    };
    let total_engines = state.translation.service.enabled_engine_count().await;

    match state
        .translation
        .service
        .run_quick(
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
                    format!("（无翻译结果）\n{source}")
                } else {
                    joined
                }
            };
            let (w, h) = overlay::window_manager::estimate_mt_card_size(&display);
            let place = mt_place(pos, bounds, w, h);
            let payload =
                overlay::translate_card::TranslateCardData::Mt(overlay::translate_card::MtCardData {
                    source: source.to_string(),
                    from: from.clone(),
                    to: to.clone(),
                    response: resp,
                    total_engines,
                });
            if let Err(e) = overlay::translate_card::show_translate_card(
                app, &payload, place.x, place.y, w, h, opts,
            )
            .await
            {
                tracing::warn!("[present] mt card failed: {e}");
            }
        },
        Err(e) => {
            tracing::warn!("[present] mt failed: {e}");
            let payload = overlay::translate_card::TranslateCardData::Mt(
                overlay::translate_card::MtCardData {
                    source: source.to_string(),
                    from: from.clone(),
                    to: to.clone(),
                    response: TranslateResponse {
                        results: vec![],
                        detected_language: None,
                        errors: vec![format!("{e}")],
                    },
                    total_engines,
                },
            );
            let place = mt_place(pos, bounds, 320.0, 120.0);
            if let Err(e2) = overlay::translate_card::show_translate_card(
                app, &payload, place.x, place.y, 320.0, 120.0, opts,
            )
            .await
            {
                tracing::warn!("[present] mt fail card failed: {e2}");
            }
        },
    }
}

/// MT card placement: near the selection (below, or above when near the screen
/// bottom) so it never occludes the target; falls back to the cursor.
fn mt_place(
    pos: Option<(f64, f64)>,
    bounds: Option<&crate::selection::SelectionBounds>,
    w: f64,
    h: f64,
) -> overlay::OverlayPosition {
    if let Some(b) = bounds {
        let (cx, cy) = pos.unwrap_or_else(cursor_pos);
        let (x, y) = overlay::positioner::place_near_bounds(b, w, h, cx, cy);
        overlay::OverlayPosition::new(x, y, w, h)
    } else {
        let (cx, cy) = pos.unwrap_or_else(cursor_pos);
        overlay::OverlayPosition::at_cursor(cx, cy)
    }
}

/// Hover path: dictionary only; miss / junk → silent (never MT).
/// `bounds`: when present, the card is placed below the word instead of at
/// the cursor, so it doesn't occlude the word being hovered.
pub async fn present_hover_dictionary(
    app: &AppHandle,
    word: &str,
    x: f64,
    y: f64,
    bounds: Option<&crate::selection::SelectionBounds>,
) {
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
            present_selection(app, word, bounds, false).await;
            return;
        },
        _ => {},
    }
    let Some(card) = lookup_word(app, word).await else {
        // User choice: on hover dictionary miss, return the LLM translation
        // result instead of showing nothing. Pass the hovered word's bounds so
        // the card is placed below it (not under the cursor) and keeps the word
        // alive while the cursor parks on it (P1: was CJK-only; now any miss).
        present_mt_card(app, word, Some((x, y)), bounds, false).await;
        return;
    };
    #[cfg(windows)]
    if crate::selection::mouse_hook::key_pressed_within_ms(400) {
        return;
    }
    present_dict_card(app, &card, Some((x, y)), bounds, false).await;
}

/// Unified selection / pop / hotkey path.
/// Word → dict card (miss silent, no MT junk); phrase/sentence → MT; junk → reject.
/// `bounds`: passed through to the card so it appears below the selection
/// rather than at the cursor (Fix 3).
/// `steal_focus`: true for user-initiated (pop click / auto-on-select / hotkey);
/// false for hover-driven sentence cards.
pub async fn present_selection(
    app: &AppHandle,
    text: &str,
    bounds: Option<&crate::selection::SelectionBounds>,
    steal_focus: bool,
) {
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
        "[pop] pending_len={} preview={:?} route={} steal_focus={}",
        trimmed.chars().count(),
        preview,
        route,
        steal_focus
    );

    match class {
        TextClass::Junk => {},
        TextClass::Word => {
            if let Some(card) = lookup_word(app, trimmed).await {
                present_dict_card(app, &card, None, bounds, steal_focus).await;
            } else {
                tracing::info!("[present] word dict miss — no MT");
            }
        },
        TextClass::Phrase | TextClass::Sentence => {
            // Short CJK/English phrase that is still is_single_word → try dict first
            if dictionary::is_single_word(trimmed) && trimmed.chars().count() <= 32 {
                if let Some(card) = lookup_word(app, trimmed).await {
                    present_dict_card(app, &card, None, bounds, steal_focus).await;
                    return;
                }
            }
            present_mt_card(app, trimmed, None, bounds, steal_focus).await;
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

use crate::furigana::{self, FuriganaSegment};
use tauri::command;

/// Add furigana annotations to Japanese text.
/// Returns segments with reading information for kanji characters.
#[command]
pub async fn add_furigana(text: String) -> Result<Vec<FuriganaSegment>, String> {
    if text.trim().is_empty() {
        return Ok(vec![]);
    }

    furigana::add_furigana(&text)
}

/// Add furigana and return as HTML with ruby annotations.
#[command]
pub async fn add_furigana_html(text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }

    let segments = furigana::add_furigana(&text)?;
    Ok(furigana::segments_to_html(&segments))
}

/// Add furigana and return as text with parenthesized readings.
#[command]
pub async fn add_furigana_text(text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }

    let segments = furigana::add_furigana(&text)?;
    Ok(furigana::segments_to_text(&segments))
}

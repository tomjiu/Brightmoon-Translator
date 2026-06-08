use lindera_dictionary::{DictionaryConfig, DictionaryKind};
use lindera_tokenizer::tokenizer::{Tokenizer, TokenizerConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuriganaSegment {
    /// Original surface form (kanji/kana)
    pub surface: String,
    /// Reading in katakana (only for kanji segments)
    pub reading: Option<String>,
    /// Whether this segment contains kanji
    pub has_kanji: bool,
}

/// Add furigana annotations to Japanese text.
/// Returns a list of segments, each with optional reading information.
pub fn add_furigana(text: &str) -> Result<Vec<FuriganaSegment>, String> {
    if text.trim().is_empty() {
        return Ok(vec![]);
    }

    // Create tokenizer with IPAdic dictionary
    let config = TokenizerConfig {
        dictionary: DictionaryConfig {
            kind: Some(DictionaryKind::IPADIC),
            path: None,
        },
        user_dictionary: None,
        mode: lindera_core::mode::Mode::Normal,
    };

    let tokenizer = Tokenizer::from_config(config)
        .map_err(|e| format!("Failed to create tokenizer: {}", e))?;

    let tokens = tokenizer
        .tokenize(text)
        .map_err(|e| format!("Failed to tokenize: {}", e))?;

    let mut segments = Vec::new();

    for mut token in tokens {
        let surface = token.text.to_string();

        // Check if the surface form contains kanji
        let has_kanji = surface.chars().any(is_kanji);

        // Get reading from token details
        let reading = if has_kanji {
            if let Some(details) = token.get_details() {
                // IPAdic format: [POS, POS1, POS2, ... , reading, ...]
                // Index 7 is typically the reading in katakana
                if details.len() > 7 {
                    let raw_reading = details[7].to_string();
                    if raw_reading == "*" || raw_reading.is_empty() {
                        None
                    } else {
                        Some(katakana_to_hiragana(&raw_reading))
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        segments.push(FuriganaSegment {
            surface,
            reading,
            has_kanji,
        });
    }

    Ok(segments)
}

/// Convert segments to HTML with ruby annotations
pub fn segments_to_html(segments: &[FuriganaSegment]) -> String {
    let mut html = String::new();

    for seg in segments {
        if seg.has_kanji {
            if let Some(ref reading) = seg.reading {
                if reading != &seg.surface && !reading.is_empty() {
                    html.push_str(&format!(
                        "<ruby>{}<rp>(</rp><rt>{}</rt><rp>)</rp></ruby>",
                        html_escape(&seg.surface),
                        html_escape(reading)
                    ));
                    continue;
                }
            }
        }
        html.push_str(&html_escape(&seg.surface));
    }

    html
}

/// Convert segments to a simple text representation with parentheses
pub fn segments_to_text(segments: &[FuriganaSegment]) -> String {
    let mut result = String::new();

    for seg in segments {
        if seg.has_kanji {
            if let Some(ref reading) = seg.reading {
                if reading != &seg.surface && !reading.is_empty() {
                    result.push_str(&format!("{}({})", seg.surface, reading));
                    continue;
                }
            }
        }
        result.push_str(&seg.surface);
    }

    result
}

/// Check if a character is a CJK kanji
fn is_kanji(c: char) -> bool {
    let code = c as u32;
    matches!(code,
        0x4E00..=0x9FFF |      // CJK Unified Ideographs
        0x3400..=0x4DBF |      // CJK Unified Ideographs Extension A
        0x20000..=0x2A6DF      // CJK Unified Ideographs Extension B
    )
}

/// Convert katakana to hiragana
fn katakana_to_hiragana(text: &str) -> String {
    text.chars()
        .map(|c| {
            let code = c as u32;
            // Katakana range: 0x30A0-0x30FF
            // Hiragana range: 0x3040-0x309F
            // Difference: 0x60
            if (0x30A1..=0x30F6).contains(&code) {
                char::from_u32(code - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// HTML escape helper
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_furigana_basic() {
        let result = add_furigana("日本語の勉強").unwrap();
        assert!(!result.is_empty());
        // Should have readings for kanji segments
        let kanji_segments: Vec<_> = result.iter().filter(|s| s.has_kanji).collect();
        assert!(!kanji_segments.is_empty());
    }

    #[test]
    fn test_furigana_empty() {
        let result = add_furigana("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_katakana_to_hiragana() {
        assert_eq!(katakana_to_hiragana("ニホン"), "にほん");
        assert_eq!(katakana_to_hiragana("テスト"), "てすと");
    }

    #[test]
    fn test_segments_to_html() {
        let segments = vec![
            FuriganaSegment {
                surface: "日本".to_string(),
                reading: Some("にほん".to_string()),
                has_kanji: true,
            },
            FuriganaSegment {
                surface: "語".to_string(),
                reading: Some("ご".to_string()),
                has_kanji: true,
            },
        ];
        let html = segments_to_html(&segments);
        assert!(html.contains("<ruby>"));
        assert!(html.contains("<rt>にほん</rt>"));
    }
}

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Quality score for a single translation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationScore {
    /// Overall score 1-5 (mapped from 0-100)
    pub overall: f64,
    /// BLEU-like n-gram match score (0-100)
    pub bleu_approx: f64,
    /// Length ratio score (0-100)
    pub length_ratio: f64,
    /// Terminology consistency score (0-100)
    pub terminology: f64,
    /// Fluency score (0-100)
    pub fluency: f64,
    /// Detailed breakdown messages
    pub details: Vec<String>,
}

/// Engine comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineScore {
    /// Engine name
    pub engine: String,
    /// Translated text
    pub translated: String,
    /// Quality score
    pub score: TranslationScore,
}

/// Weights for combining individual scores
const WEIGHT_BLEU: f64 = 0.30;
const WEIGHT_LENGTH: f64 = 0.15;
const WEIGHT_TERMINOLOGY: f64 = 0.25;
const WEIGHT_FLUENCY: f64 = 0.30;

/// Calculate translation quality score
pub fn score_translation(
    original: &str,
    translated: &str,
    lang_pair: &str,
    glossary: Option<&HashMap<String, Vec<crate::models::glossary::GlossaryEntry>>>,
) -> TranslationScore {
    let mut details = Vec::new();

    // 1. BLEU approximation (n-gram overlap)
    let bleu = calculate_bleu_approx(original, translated, &mut details);

    // 2. Length ratio score
    let length = calculate_length_score(original, translated, lang_pair, &mut details);

    // 3. Terminology consistency
    let terminology = calculate_terminology_score(original, translated, lang_pair, glossary, &mut details);

    // 4. Fluency check
    let fluency = calculate_fluency_score(translated, lang_pair, &mut details);

    // Weighted average
    let weighted = bleu * WEIGHT_BLEU
        + length * WEIGHT_LENGTH
        + terminology * WEIGHT_TERMINOLOGY
        + fluency * WEIGHT_FLUENCY;

    // Map 0-100 to 1-5 scale
    let overall = map_to_stars(weighted);

    TranslationScore {
        overall,
        bleu_approx: round2(bleu),
        length_ratio: round2(length),
        terminology: round2(terminology),
        fluency: round2(fluency),
        details,
    }
}

/// BLEU-like n-gram overlap approximation
fn calculate_bleu_approx(original: &str, translated: &str, details: &mut Vec<String>) -> f64 {
    if original.is_empty() || translated.is_empty() {
        details.push("Empty text, BLEU score set to 0".to_string());
        return 0.0;
    }

    // Tokenize into character n-grams (better for CJK languages)
    let orig_ngrams = extract_char_ngrams(original, 2);
    let trans_ngrams = extract_char_ngrams(translated, 2);

    if orig_ngrams.is_empty() || trans_ngrams.is_empty() {
        return 50.0; // Default neutral score
    }

    // Count matching n-grams
    let orig_set: HashSet<&str> = orig_ngrams.iter().map(|s| s.as_str()).collect();
    let trans_set: HashSet<&str> = trans_ngrams.iter().map(|s| s.as_str()).collect();

    let intersection_count = orig_set.iter().filter(|n| trans_set.contains(*n)).count();
    let precision = if trans_set.is_empty() {
        0.0
    } else {
        intersection_count as f64 / trans_set.len() as f64
    };

    // Brevity penalty
    let bp = if translated.len() < original.len() {
        let ratio = translated.len() as f64 / original.len() as f64;
        (1.0 - ratio).max(0.0).powi(2)
    } else {
        0.0
    };

    let score = (precision - bp).max(0.0) * 100.0;
    details.push(format!(
        "N-gram overlap: {:.1}%, brevity penalty: {:.1}%",
        precision * 100.0,
        bp * 100.0
    ));

    score.min(100.0)
}

/// Extract character-level n-grams
fn extract_char_ngrams(text: &str, n: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < n {
        return vec![text.to_string()];
    }
    chars
        .windows(n)
        .map(|w| w.iter().collect())
        .collect()
}

/// Length ratio score - penalize too short or too long translations
fn calculate_length_score(
    original: &str,
    translated: &str,
    lang_pair: &str,
    details: &mut Vec<String>,
) -> f64 {
    if original.is_empty() || translated.is_empty() {
        return 50.0;
    }

    let orig_len = original.chars().count();
    let trans_len = translated.chars().count();

    if orig_len == 0 || trans_len == 0 {
        return 50.0;
    }

    // Expected ratio depends on language pair
    let expected_ratio = get_expected_length_ratio(lang_pair);
    let actual_ratio = trans_len as f64 / orig_len as f64;

    // Calculate deviation from expected ratio
    let deviation = (actual_ratio - expected_ratio).abs() / expected_ratio;

    // Score: 100 when perfect, decreasing with deviation
    let score = (100.0 * (1.0 - deviation.min(1.0))).max(0.0);

    details.push(format!(
        "Length ratio: {:.2} (expected {:.2}), deviation: {:.1}%",
        actual_ratio,
        expected_ratio,
        deviation * 100.0
    ));

    score
}

/// Expected character length ratio for language pairs
fn get_expected_length_ratio(lang_pair: &str) -> f64 {
    let parts: Vec<&str> = lang_pair.split('-').collect();
    if parts.len() != 2 {
        return 1.0;
    }
    let (from, to) = (parts[0], parts[1]);

    let from_is_cjk = matches!(from, "zh" | "ja" | "ko");
    let to_is_cjk = matches!(to, "zh" | "ja" | "ko");

    match (from_is_cjk, to_is_cjk) {
        // CJK to CJK
        (true, true) => 1.0,
        // European to CJK (European text is shorter in characters)
        (false, true) => 0.6,
        // CJK to European (CJK text uses fewer characters)
        (true, false) => 1.8,
        // European to European
        (false, false) => 1.0,
    }
}

/// Terminology consistency score
fn calculate_terminology_score(
    original: &str,
    translated: &str,
    lang_pair: &str,
    glossary: Option<&HashMap<String, Vec<crate::models::glossary::GlossaryEntry>>>,
    details: &mut Vec<String>,
) -> f64 {
    let glossary = match glossary {
        Some(g) => g,
        None => {
            details.push("No glossary loaded, terminology score neutral".to_string());
            return 75.0; // Neutral score when no glossary
        }
    };

    let entries = match glossary.get(lang_pair) {
        Some(e) if !e.is_empty() => e,
        _ => {
            details.push(format!("No glossary entries for {}", lang_pair));
            return 75.0;
        }
    };

    // Find terms that appear in original
    let mut matched = 0;
    let mut total = 0;

    for entry in entries {
        if original.contains(&entry.source) {
            total += 1;
            // Check if target appears in translation
            if translated.contains(&entry.target) {
                matched += 1;
            }
        }
    }

    if total == 0 {
        details.push("No glossary terms found in source text".to_string());
        return 75.0;
    }

    let score = (matched as f64 / total as f64) * 100.0;
    details.push(format!(
        "Glossary terms: {}/{} matched ({:.0}%)",
        matched,
        total,
        score
    ));

    score
}

/// Fluency score - check for common issues
fn calculate_fluency_score(translated: &str, lang_pair: &str, details: &mut Vec<String>) -> f64 {
    if translated.is_empty() {
        return 0.0;
    }

    let mut penalties = 0.0;

    // 1. Check for repeated characters (e.g., "哈哈哈哈哈哈")
    let chars: Vec<char> = translated.chars().collect();
    let mut max_repeat = 1;
    let mut current_repeat = 1;
    for i in 1..chars.len() {
        if chars[i] == chars[i - 1] {
            current_repeat += 1;
            max_repeat = max_repeat.max(current_repeat);
        } else {
            current_repeat = 1;
        }
    }
    if max_repeat > 5 {
        penalties += 20.0;
        details.push(format!("Excessive character repetition ({})", max_repeat));
    }

    // 2. Check for mixed language issues (random CJK in European text or vice versa)
    let to_lang = lang_pair.split('-').nth(1).unwrap_or("en");
    let has_cjk = translated.chars().any(|c| is_cjk(c));
    let has_latin = translated.chars().any(|c| c.is_ascii_alphabetic());

    let mixed_penalty = match to_lang {
        "zh" | "ja" | "ko" => {
            // For CJK target, some Latin is OK (proper nouns)
            if has_latin && !has_cjk {
                30.0 // Entirely Latin for CJK target is bad
            } else {
                0.0
            }
        }
        "en" | "fr" | "de" | "es" | "ru" => {
            // For European target, CJK characters are usually wrong
            if has_cjk {
                25.0
            } else {
                0.0
            }
        }
        _ => 0.0,
    };

    if mixed_penalty > 0.0 {
        penalties += mixed_penalty;
        details.push("Mixed language issue detected".to_string());
    }


    // 3. Check for common punctuation issues
    let has_double_space = translated.contains("  ");
    let has_mismatched_brackets = check_bracket_balance(translated);

    if has_double_space {
        penalties += 5.0;
        details.push("Double spaces detected".to_string());
    }

    if has_mismatched_brackets {
        penalties += 10.0;
        details.push("Mismatched brackets/quotes".to_string());
    }

    // 4. Check if translation is identical to source (possible failure)
    if translated.trim() == translated.trim() && translated.chars().count() > 5 {
        // Same text might be intentional for proper nouns
        // Only penalize if significant length
        let similarity = jaccard_similarity(translated, translated);
        if similarity > 0.95 && translated.chars().count() > 20 {
            penalties += 15.0;
            details.push("Translation very similar to source".to_string());
        }
    }

    let score = (100.0_f64 - penalties).max(0.0_f64);
    details.push(format!("Fluency base score: {:.0}", score));

    score
}

/// Check if brackets and quotes are balanced
fn check_bracket_balance(text: &str) -> bool {
    let mut stack = Vec::new();
    for c in text.chars() {
        match c {
            '(' | '[' | '{' => stack.push(c),
            ')' => {
                if stack.pop() != Some('(') {
                    return true;
                }
            }
            ']' => {
                if stack.pop() != Some('[') {
                    return true;
                }
            }
            '}' => {
                if stack.pop() != Some('{') {
                    return true;
                }
            }
            _ => {}
        }
    }
    !stack.is_empty()
}

/// Check if character is CJK
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' |  // CJK Unified Ideographs
        '\u{3040}'..='\u{309F}' |  // Hiragana
        '\u{30A0}'..='\u{30FF}' |  // Katakana
        '\u{AC00}'..='\u{D7AF}' |  // Hangul
        '\u{F900}'..='\u{FAFF}'    // CJK Compatibility Ideographs
    )
}

/// Jaccard similarity between two strings (character-level)
fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let set_a: HashSet<char> = a.chars().collect();
    let set_b: HashSet<char> = b.chars().collect();

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Map 0-100 score to 1-5 stars
fn map_to_stars(score: f64) -> f64 {
    let stars = 1.0 + (score / 100.0) * 4.0;
    (stars * 2.0).round() / 2.0 // Round to nearest 0.5
}

/// Round to 2 decimal places
fn round2(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bleu_approx_identical() {
        let mut details = Vec::new();
        let score = calculate_bleu_approx("hello world", "hello world", &mut details);
        assert!(score > 80.0, "Identical texts should score high: {}", score);
    }

    #[test]
    fn test_bleu_approx_empty() {
        let mut details = Vec::new();
        let score = calculate_bleu_approx("", "hello", &mut details);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_length_score_balanced() {
        let mut details = Vec::new();
        let score = calculate_length_score("hello", "你好", "en-zh", &mut details);
        assert!(score > 50.0, "Reasonable length should score OK: {}", score);
    }

    #[test]
    fn test_fluency_score_clean() {
        let mut details = Vec::new();
        let score = calculate_fluency_score("这是一段正常的翻译", "en-zh", &mut details);
        assert!(score > 80.0, "Clean text should score high: {}", score);
    }

    #[test]
    fn test_fluency_score_repeated() {
        let mut details = Vec::new();
        let score = calculate_fluency_score("哈哈哈哈哈哈哈哈哈哈哈哈哈", "en-zh", &mut details);
        assert!(score < 80.0, "Repeated text should score lower: {}", score);
    }

    #[test]
    fn test_map_to_stars() {
        assert_eq!(map_to_stars(0.0), 1.0);
        assert_eq!(map_to_stars(50.0), 3.0);
        assert_eq!(map_to_stars(100.0), 5.0);
    }

    #[test]
    fn test_overall_score() {
        let score = score_translation(
            "Hello, how are you?",
            "你好，你怎么样？",
            "en-zh",
            None,
        );
        assert!(score.overall >= 1.0 && score.overall <= 5.0);
        assert!(score.fluency > 0.0);
    }

    #[test]
    fn test_cjk_detection() {
        assert!(is_cjk('你'));
        assert!(is_cjk('あ'));
        assert!(is_cjk('가'));
        assert!(!is_cjk('a'));
    }

    #[test]
    fn test_bracket_balance() {
        assert!(!check_bracket_balance("(hello)"));
        assert!(check_bracket_balance("(hello"));
        assert!(check_bracket_balance(")hello("));
    }
}

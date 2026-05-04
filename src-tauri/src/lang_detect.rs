use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub language: String,
    pub confidence: f32,
    pub name: String,
}

/// Detect language based on character patterns
pub fn detect_language(text: &str) -> DetectionResult {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return DetectionResult {
            language: "unknown".to_string(),
            confidence: 0.0,
            name: "未知".to_string(),
        };
    }

    let chars: Vec<char> = trimmed.chars().collect();
    let total = chars.len() as f32;

    // Count character categories
    let mut cjk_count = 0;
    let mut hiragana_count = 0;
    let mut katakana_count = 0;
    let mut hangul_count = 0;
    let mut latin_count = 0;
    let mut cyrillic_count = 0;
    let mut arabic_count = 0;
    let mut thai_count = 0;

    for &c in &chars {
        let code = c as u32;
        match code {
            // CJK Unified Ideographs
            0x4E00..=0x9FFF => cjk_count += 1,
            0x3400..=0x4DBF => cjk_count += 1,
            0x20000..=0x2A6DF => cjk_count += 1,
            // Hiragana
            0x3040..=0x309F => hiragana_count += 1,
            // Katakana
            0x30A0..=0x30FF => katakana_count += 1,
            0x31F0..=0x31FF => katakana_count += 1,
            // Hangul
            0xAC00..=0xD7AF => hangul_count += 1,
            0x1100..=0x11FF => hangul_count += 1,
            0x3130..=0x318F => hangul_count += 1,
            // Latin
            0x0041..=0x005A => latin_count += 1,
            0x0061..=0x007A => latin_count += 1,
            0x00C0..=0x024F => latin_count += 1,
            // Cyrillic
            0x0400..=0x04FF => cyrillic_count += 1,
            // Arabic
            0x0600..=0x06FF => arabic_count += 1,
            0x0750..=0x077F => arabic_count += 1,
            // Thai
            0x0E00..=0x0E7F => thai_count += 1,
            _ => {}
        }
    }

    // Determine language based on character distribution
    let cjk_ratio = cjk_count as f32 / total;
    let hiragana_ratio = hiragana_count as f32 / total;
    let katakana_ratio = katakana_count as f32 / total;
    let hangul_ratio = hangul_count as f32 / total;
    let latin_ratio = latin_count as f32 / total;
    let cyrillic_ratio = cyrillic_count as f32 / total;
    let arabic_ratio = arabic_count as f32 / total;
    let thai_ratio = thai_count as f32 / total;

    // Japanese: mix of CJK + Hiragana/Katakana
    if (hiragana_ratio + katakana_ratio) > 0.1 {
        return DetectionResult {
            language: "ja".to_string(),
            confidence: (hiragana_ratio + katakana_ratio + cjk_ratio).min(1.0),
            name: "日本語".to_string(),
        };
    }

    // Korean: Hangul
    if hangul_ratio > 0.3 {
        return DetectionResult {
            language: "ko".to_string(),
            confidence: hangul_ratio.min(1.0),
            name: "한국어".to_string(),
        };
    }

    // Chinese: CJK without Japanese/Korean indicators
    if cjk_ratio > 0.3 {
        return DetectionResult {
            language: "zh".to_string(),
            confidence: cjk_ratio.min(1.0),
            name: "中文".to_string(),
        };
    }

    // Russian: Cyrillic
    if cyrillic_ratio > 0.3 {
        return DetectionResult {
            language: "ru".to_string(),
            confidence: cyrillic_ratio.min(1.0),
            name: "Русский".to_string(),
        };
    }

    // Arabic
    if arabic_ratio > 0.3 {
        return DetectionResult {
            language: "ar".to_string(),
            confidence: arabic_ratio.min(1.0),
            name: "العربية".to_string(),
        };
    }

    // Thai
    if thai_ratio > 0.3 {
        return DetectionResult {
            language: "th".to_string(),
            confidence: thai_ratio.min(1.0),
            name: "ไทย".to_string(),
        };
    }

    // Default to English for Latin script
    if latin_ratio > 0.5 {
        return DetectionResult {
            language: "en".to_string(),
            confidence: latin_ratio.min(1.0),
            name: "English".to_string(),
        };
    }

    // Fallback: try to guess based on common words
    let lower = trimmed.to_lowercase();
    if contains_english_words(&lower) {
        return DetectionResult {
            language: "en".to_string(),
            confidence: 0.6,
            name: "English".to_string(),
        };
    }

    DetectionResult {
        language: "auto".to_string(),
        confidence: 0.0,
        name: "自动检测".to_string(),
    }
}

fn contains_english_words(text: &str) -> bool {
    let common_words = [
        "the", "is", "at", "which", "on", "and", "a", "to", "in", "it", "of", "for", "that",
        "this", "with", "you", "but", "have", "not", "are", "be", "from", "or", "by", "one",
        "had", "was", "what", "when", "where", "how", "all", "can", "her", "there", "been",
        "if", "will", "do",
    ];

    let words: Vec<&str> = text.split_whitespace().collect();
    let mut match_count = 0;

    for word in &words {
        let clean_word: String = word.chars().filter(|c| c.is_alphabetic()).collect();
        if common_words.contains(&clean_word.as_str()) {
            match_count += 1;
        }
    }

    match_count > 2 || (words.len() > 3 && match_count as f32 / words.len() as f32 > 0.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_chinese() {
        let result = detect_language("你好世界");
        assert_eq!(result.language, "zh");
    }

    #[test]
    fn test_detect_japanese() {
        let result = detect_language("こんにちは世界");
        assert_eq!(result.language, "ja");
    }

    #[test]
    fn test_detect_korean() {
        let result = detect_language("안녕하세요 세계");
        assert_eq!(result.language, "ko");
    }

    #[test]
    fn test_detect_english() {
        let result = detect_language("Hello World");
        assert_eq!(result.language, "en");
    }

    #[test]
    fn test_detect_russian() {
        let result = detect_language("Привет мир");
        assert_eq!(result.language, "ru");
    }
}

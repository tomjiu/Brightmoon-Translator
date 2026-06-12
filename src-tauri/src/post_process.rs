use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementRule {
    pub id: String,
    pub pattern: String,
    pub replacement: String,
    pub enabled: bool,
    pub is_regex: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostProcessConfig {
    pub rules: Vec<ReplacementRule>,
    pub trim_whitespace: bool,
    pub fix_punctuation: bool,
    pub fix_newlines: bool,
    pub auto_correct: bool,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            trim_whitespace: true,
            fix_punctuation: true,
            fix_newlines: true,
            auto_correct: true,
        }
    }
}

/// Result of auto-correction analysis
#[derive(Debug, Clone)]
pub struct AutoCorrectResult {
    pub corrected: String,
    pub warnings: Vec<String>,
}

/// Detect if translated text is likely untranslated (same as source).
/// Returns a similarity ratio between 0.0 (completely different) and 1.0 (identical).
fn text_similarity(a: &str, b: &str) -> f32 {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    if a_chars.is_empty() && b_chars.is_empty() {
        return 1.0;
    }
    if a_chars.is_empty() || b_chars.is_empty() {
        return 0.0;
    }

    // Quick check: if lengths are very different, texts are likely different
    let len_ratio =
        a_chars.len().min(b_chars.len()) as f32 / a_chars.len().max(b_chars.len()) as f32;
    if len_ratio < 0.3 {
        return 0.0;
    }

    // Character-level Jaccard similarity on bigrams
    let a_bigrams = char_bigrams(&a_chars);
    let b_bigrams = char_bigrams(&b_chars);

    if a_bigrams.is_empty() && b_bigrams.is_empty() {
        return if a_chars == b_chars { 1.0 } else { 0.0 };
    }
    if a_bigrams.is_empty() || b_bigrams.is_empty() {
        return 0.0;
    }

    let intersection = a_bigrams.iter().filter(|bg| b_bigrams.contains(bg)).count();
    let union = a_bigrams.len() + b_bigrams.len() - intersection;

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

fn char_bigrams(chars: &[char]) -> Vec<(char, char)> {
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Detect garbled characters (mojibake) in text.
/// Returns true if the text appears to contain encoding artifacts.
fn has_garbled_chars(text: &str) -> bool {
    // Check for Unicode replacement character
    if text.contains('\u{FFFD}') {
        return true;
    }

    // Check for common UTF-8 → Latin-1 mojibake patterns
    // These occur when UTF-8 bytes are decoded as Latin-1
    let mojibake_patterns = [
        "Ã¡",
        "Ã©",
        "Ã­",
        "Ã³",
        "Ãº", // Spanish accented vowels
        "Ã¤",
        "Ã¶",
        "Ã¼",
        "ÃŸ", // German umlauts
        "Ã ",
        "Ã¨",
        "Ã¬",
        "Ã²",
        "Ã¹", // French/Italian accented vowels
        "Ã§",
        "Ã±", // c-cedilla, n-tilde
        "Ð",
        "Ñ", // Cyrillic mojibake start bytes
        "â€™",
        "â€œ",
        "â€",
        "â€¦", // Smart quotes mojibake
        "Ã¢Â€Â™",
        "Ã¢Â€Âœ", // Double-encoded mojibake
    ];

    for pattern in &mojibake_patterns {
        if text.contains(pattern) {
            return true;
        }
    }

    // Check for excessive control characters (except common whitespace)
    let control_count = text
        .chars()
        .filter(|c| {
            let code = *c as u32;
            code < 0x20 && !matches!(code, 0x09 | 0x0A | 0x0D) // tab, LF, CR are OK
        })
        .count();

    if control_count > 0 {
        return true;
    }

    // Check for Private Use Area characters (often indicate font-specific glyphs that didn't render)
    let pua_count = text
        .chars()
        .filter(|c| {
            let code = *c as u32;
            (0xE000..=0xF8FF).contains(&code) || (0xF0000..=0xFFFFF).contains(&code)
        })
        .count();

    // If more than 5% PUA characters, likely garbled
    if !text.is_empty() && pua_count as f32 / text.chars().count() as f32 > 0.05 {
        return true;
    }

    false
}

pub struct PostProcessor {
    config: Mutex<PostProcessConfig>,
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!("Failed to create config directory {:?}: {}", path, e);
    }
    path.push("post_process.json");
    path
}

impl PostProcessor {
    pub fn load() -> Self {
        let path = config_path();
        let config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
                    tracing::error!("Failed to parse post-process config {:?}: {}", path, e);
                    PostProcessConfig::default()
                }),
                Err(e) => {
                    tracing::error!("Failed to read post-process config {:?}: {}", path, e);
                    PostProcessConfig::default()
                },
            }
        } else {
            PostProcessConfig::default()
        };

        Self {
            config: Mutex::new(config),
        }
    }

    pub fn save(&self) {
        let config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        let path = config_path();
        match serde_json::to_string_pretty(&*config) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    tracing::error!("Failed to save post-process config {:?}: {}", path, e);
                }
            },
            Err(e) => {
                tracing::error!("Failed to serialize post-process config: {}", e);
            },
        }
    }

    pub fn get_config(&self) -> PostProcessConfig {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn update_config(&self, config: PostProcessConfig) {
        let mut current = self.config.lock().unwrap_or_else(|e| e.into_inner());
        *current = config;
        drop(current);
        self.save();
    }

    pub fn add_rule(&self, rule: ReplacementRule) {
        let mut config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        config.rules.push(rule);
        drop(config);
        self.save();
    }

    pub fn remove_rule(&self, id: &str) {
        let mut config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        config.rules.retain(|r| r.id != id);
        drop(config);
        self.save();
    }

    pub fn update_rule(&self, id: &str, rule: ReplacementRule) {
        let mut config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = config.rules.iter_mut().find(|r| r.id == id) {
            *existing = rule;
        }
        drop(config);
        self.save();
    }

    pub fn process(&self, text: &str) -> String {
        let config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        let mut result = text.to_string();

        // Apply replacement rules
        for rule in &config.rules {
            if !rule.enabled {
                continue;
            }

            if rule.is_regex {
                if let Ok(re) = regex::Regex::new(&rule.pattern) {
                    result = re.replace_all(&result, &rule.replacement).to_string();
                }
            } else {
                result = result.replace(&rule.pattern, &rule.replacement);
            }
        }

        // Fix punctuation
        if config.fix_punctuation {
            result = fix_punctuation(&result);
        }

        // Fix newlines
        if config.fix_newlines {
            result = fix_newlines(&result);
        }

        // Trim whitespace
        if config.trim_whitespace {
            result = result.trim().to_string();
        }

        result
    }

    /// Auto-correct translation output: detect untranslated text and garbled characters.
    /// Returns the (possibly corrected) text along with any warnings.
    pub fn auto_correct(
        &self,
        translated: &str,
        source: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> AutoCorrectResult {
        let config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        if !config.auto_correct {
            return AutoCorrectResult {
                corrected: translated.to_string(),
                warnings: Vec::new(),
            };
        }
        drop(config);

        let mut warnings = Vec::new();
        let mut result = translated.to_string();

        // 1. Garbled character detection
        if has_garbled_chars(&result) {
            warnings.push("翻译结果可能包含乱码字符".to_string());
            // Remove replacement characters
            result = result.replace('\u{FFFD}', "");
            // Clean up excessive control characters
            result = result
                .chars()
                .filter(|c| {
                    let code = *c as u32;
                    code >= 0x20 || matches!(code, 0x09 | 0x0A | 0x0D)
                })
                .collect();
            result = result.trim().to_string();
        }

        // 2. Untranslated text detection
        // Only check when source and target languages differ
        if source_lang != target_lang && !source.is_empty() && !result.is_empty() {
            let similarity = text_similarity(source, &result);
            // Threshold: 0.85 means very similar texts are flagged as likely untranslated
            if similarity > 0.85 {
                warnings.push(format!(
                    "译文与原文相似度 {:.0}%，可能未正确翻译",
                    similarity * 100.0
                ));
            }
        }

        // 3. Check for completely empty result after cleaning
        if result.is_empty() && !source.is_empty() {
            warnings.push("翻译结果为空".to_string());
            result = source.to_string(); // Fallback to source
        }

        AutoCorrectResult {
            corrected: result,
            warnings,
        }
    }
}

fn fix_punctuation(text: &str) -> String {
    let mut result = text.to_string();

    // Fix multiple spaces
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }

    // Fix spaces before punctuation
    result = result.replace(" .", ".");
    result = result.replace(" ,", ",");
    result = result.replace(" !", "!");
    result = result.replace(" ?", "?");
    result = result.replace(" ;", ";");
    result = result.replace(" :", ":");

    // Fix multiple punctuation
    while result.contains("...") && result.contains("....") {
        result = result.replace("....", "...");
    }

    result
}

fn fix_newlines(text: &str) -> String {
    let mut result = String::new();
    let mut prev_empty = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_empty {
                result.push('\n');
                prev_empty = true;
            }
        } else {
            result.push_str(trimmed);
            result.push('\n');
            prev_empty = false;
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_punctuation_removes_spaces_before_punctuation() {
        assert_eq!(fix_punctuation("hello ."), "hello.");
        assert_eq!(fix_punctuation("hello , world"), "hello, world");
        assert_eq!(fix_punctuation("what !"), "what!");
        assert_eq!(fix_punctuation("why ?"), "why?");
    }

    #[test]
    fn test_fix_punctuation_collapses_multiple_spaces() {
        assert_eq!(fix_punctuation("hello  world"), "hello world");
        assert_eq!(fix_punctuation("a   b   c"), "a b c");
    }

    #[test]
    fn test_fix_newlines_collapses_multiple_empty_lines() {
        // Multiple empty lines collapse to a single empty line
        let input = "line1\n\n\nline2";
        assert_eq!(fix_newlines(input), "line1\n\nline2");
    }

    #[test]
    fn test_fix_newlines_preserves_single_empty_line() {
        let input = "line1\n\nline2";
        assert_eq!(fix_newlines(input), "line1\n\nline2");
    }

    #[test]
    fn test_fix_newlines_trims_whitespace() {
        let input = "  line1  \n  line2  ";
        assert_eq!(fix_newlines(input), "line1\nline2");
    }

    #[test]
    fn test_post_processor_process_with_rules() {
        let processor = PostProcessor {
            config: Mutex::new(PostProcessConfig {
                rules: vec![ReplacementRule {
                    id: "1".to_string(),
                    pattern: "foo".to_string(),
                    replacement: "bar".to_string(),
                    enabled: true,
                    is_regex: false,
                }],
                trim_whitespace: true,
                fix_punctuation: false,
                fix_newlines: false,
                auto_correct: true,
            }),
        };
        assert_eq!(processor.process("hello foo world"), "hello bar world");
    }

    #[test]
    fn test_post_processor_skips_disabled_rules() {
        let processor = PostProcessor {
            config: Mutex::new(PostProcessConfig {
                rules: vec![ReplacementRule {
                    id: "1".to_string(),
                    pattern: "foo".to_string(),
                    replacement: "bar".to_string(),
                    enabled: false,
                    is_regex: false,
                }],
                trim_whitespace: false,
                fix_punctuation: false,
                fix_newlines: false,
                auto_correct: true,
            }),
        };
        assert_eq!(processor.process("hello foo world"), "hello foo world");
    }

    #[test]
    fn test_text_similarity_identical() {
        assert!(text_similarity("hello world", "hello world") > 0.99);
    }

    #[test]
    fn test_text_similarity_different() {
        let sim = text_similarity("hello world", "你好世界");
        assert!(sim < 0.3);
    }

    #[test]
    fn test_text_similarity_empty() {
        assert_eq!(text_similarity("", ""), 1.0);
        assert_eq!(text_similarity("hello", ""), 0.0);
        assert_eq!(text_similarity("", "hello"), 0.0);
    }

    #[test]
    fn test_text_similarity_similar() {
        // Minor translation differences should still be detected as similar
        let sim = text_similarity("The cat sat on the mat", "The cat sat on a mat");
        assert!(sim > 0.7);
    }

    #[test]
    fn test_has_garbled_chars_clean() {
        assert!(!has_garbled_chars("Hello World"));
        assert!(!has_garbled_chars("你好世界"));
        assert!(!has_garbled_chars("こんにちは"));
    }

    #[test]
    fn test_has_garbled_chars_replacement_char() {
        assert!(has_garbled_chars("Hello\u{FFFD}World"));
    }

    #[test]
    fn test_has_garbled_chars_mojibake() {
        assert!(has_garbled_chars("HÃ©llo"));
        assert!(has_garbled_chars("cafÃ©"));
    }

    #[test]
    fn test_has_garbled_chars_control_chars() {
        assert!(has_garbled_chars("Hello\x01World"));
        // But tab/LF/CR should be fine
        assert!(!has_garbled_chars("Hello\tWorld\n\r"));
    }

    #[test]
    fn test_auto_correct_garbled_detection() {
        let processor = PostProcessor {
            config: Mutex::new(PostProcessConfig {
                rules: Vec::new(),
                trim_whitespace: true,
                fix_punctuation: false,
                fix_newlines: false,
                auto_correct: true,
            }),
        };
        let result = processor.auto_correct("HÃ©llo World", "Hello World", "en", "zh");
        assert!(!result.warnings.is_empty());
        assert!(result.warnings[0].contains("乱码"));
    }

    #[test]
    fn test_auto_correct_untranslated_detection() {
        let processor = PostProcessor {
            config: Mutex::new(PostProcessConfig {
                rules: Vec::new(),
                trim_whitespace: true,
                fix_punctuation: false,
                fix_newlines: false,
                auto_correct: true,
            }),
        };
        // Same text with different languages should trigger warning
        let result = processor.auto_correct("hello world", "hello world", "en", "zh");
        assert!(result.warnings.iter().any(|w| w.contains("未正确翻译")));
    }

    #[test]
    fn test_auto_correct_same_language_no_warning() {
        let processor = PostProcessor {
            config: Mutex::new(PostProcessConfig {
                rules: Vec::new(),
                trim_whitespace: true,
                fix_punctuation: false,
                fix_newlines: false,
                auto_correct: true,
            }),
        };
        // Same language, same text = no warning (could be valid)
        let result = processor.auto_correct("hello world", "hello world", "en", "en");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_auto_correct_disabled() {
        let processor = PostProcessor {
            config: Mutex::new(PostProcessConfig {
                rules: Vec::new(),
                trim_whitespace: true,
                fix_punctuation: false,
                fix_newlines: false,
                auto_correct: false,
            }),
        };
        // Even garbled text passes through when disabled
        let result = processor.auto_correct("HÃ©llo", "Hello", "en", "zh");
        assert!(result.warnings.is_empty());
        assert_eq!(result.corrected, "HÃ©llo");
    }

    #[test]
    fn test_auto_correct_empty_after_clean() {
        let processor = PostProcessor {
            config: Mutex::new(PostProcessConfig {
                rules: Vec::new(),
                trim_whitespace: true,
                fix_punctuation: false,
                fix_newlines: false,
                auto_correct: true,
            }),
        };
        // If cleaning garbled chars leaves empty string, fallback to source
        let result = processor.auto_correct("\u{FFFD}\u{FFFD}", "Hello", "en", "zh");
        assert!(result.warnings.iter().any(|w| w.contains("乱码")));
        assert!(result.warnings.iter().any(|w| w.contains("为空")));
        assert_eq!(result.corrected, "Hello");
    }
}

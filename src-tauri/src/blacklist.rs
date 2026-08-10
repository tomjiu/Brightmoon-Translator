use std::collections::HashMap;

/// Protect blacklisted words by replacing them with placeholders before translation,
/// then restore them after translation.
pub struct BlacklistProcessor {
    blacklist: Vec<String>,
}

impl BlacklistProcessor {
    pub fn new(blacklist: Vec<String>) -> Self {
        Self { blacklist }
    }

    /// Replace blacklisted words with numbered placeholders.
    /// Single-pass implementation: scans text once, replacing blacklist words as encountered.
    /// Longest match wins for overlapping terms; ties keep blacklist order.
    /// Returns (`protected_text`, `placeholder_map`)
    pub fn protect(&self, text: &str) -> (String, HashMap<String, String>) {
        let mut result = String::with_capacity(text.len());
        let mut placeholder_map = HashMap::new();
        let mut last_match_end: usize = 0;

        for (pos, _) in text.char_indices() {
            if pos < last_match_end {
                continue;
            }

            let remaining = &text[pos..];

            let mut best_match: Option<(usize, &String)> = None;
            for (i, word) in self.blacklist.iter().enumerate() {
                if word.is_empty() {
                    continue;
                }

                if remaining.starts_with(word.as_str()) {
                    match best_match {
                        Some((best_i, best_word))
                            if word.len() < best_word.len()
                                || (word.len() == best_word.len() && i > best_i) => {},
                        _ => best_match = Some((i, word)),
                    }
                }
            }

            if let Some((i, word)) = best_match {
                if pos > last_match_end {
                    result.push_str(&text[last_match_end..pos]);
                }

                let placeholder = format!("__BLACKLIST_{i}__");
                result.push_str(&placeholder);
                placeholder_map.insert(placeholder, word.clone());
                last_match_end = pos + word.len();
            }
        }

        if last_match_end < text.len() {
            result.push_str(&text[last_match_end..]);
        }

        (result, placeholder_map)
    }

    /// Restore blacklisted words from placeholders.
    /// Computes lowercase text once, then finds all placeholder positions
    /// and builds the result in a single pass.
    pub fn restore(&self, text: &str, placeholder_map: &HashMap<String, String>) -> String {
        let lower_text = text.to_ascii_lowercase();
        let mut replacements: Vec<(usize, usize, &str)> = Vec::new();

        for (placeholder, original) in placeholder_map {
            let lower_placeholder = placeholder.to_ascii_lowercase();
            let mut search_start = 0;
            while search_start <= lower_text.len() {
                if let Some(pos) = lower_text[search_start..].find(&lower_placeholder) {
                    let abs_pos = search_start + pos;
                    replacements.push((abs_pos, placeholder.len(), original.as_str()));
                    search_start = abs_pos + placeholder.len();
                } else {
                    break;
                }
            }
        }

        if replacements.is_empty() {
            return text.to_string();
        }

        // Sort by position (forward) to build result left-to-right
        replacements.sort_by_key(|r| r.0);

        let mut result = String::with_capacity(text.len());
        let mut cursor = 0usize;
        for (pos, match_len, original) in &replacements {
            if *pos >= cursor {
                result.push_str(&text[cursor..*pos]);
                result.push_str(original);
                cursor = *pos + match_len;
            }
        }
        result.push_str(&text[cursor..]);

        result
    }

    /// Check if a word is in the blacklist
    pub fn is_blacklisted(&self, word: &str) -> bool {
        self.blacklist.iter().any(|b| b.eq_ignore_ascii_case(word))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protect_and_restore() {
        let blacklist = vec!["API".to_string(), "JavaScript".to_string()];
        let processor = BlacklistProcessor::new(blacklist);

        let text = "This is an API written in JavaScript";
        let (protected, map) = processor.protect(text);

        assert!(protected.contains("__BLACKLIST_0__"));
        assert!(protected.contains("__BLACKLIST_1__"));
        assert!(!protected.contains("API"));
        assert!(!protected.contains("JavaScript"));

        // Simulate LLM translation that might change placeholder case
        let translated = "这是一个__BLACKLIST_0__，使用__BLACKLIST_1__编写";
        let restored = processor.restore(translated, &map);

        assert_eq!(restored, "这是一个API，使用JavaScript编写");
    }

    #[test]
    fn test_is_blacklisted() {
        let blacklist = vec!["API".to_string(), "JavaScript".to_string()];
        let processor = BlacklistProcessor::new(blacklist);

        assert!(processor.is_blacklisted("API"));
        assert!(processor.is_blacklisted("api"));
        assert!(processor.is_blacklisted("Api"));
        assert!(!processor.is_blacklisted("Python"));
    }

    #[test]
    fn test_protect_empty_blacklist() {
        let processor = BlacklistProcessor::new(vec![]);
        let (protected, map) = processor.protect("Hello API World");
        assert_eq!(protected, "Hello API World");
        assert!(map.is_empty());
    }

    #[test]
    fn test_protect_empty_text() {
        let processor = BlacklistProcessor::new(vec!["API".to_string()]);
        let (protected, map) = processor.protect("");
        assert_eq!(protected, "");
        assert!(map.is_empty());
    }

    #[test]
    fn test_protect_no_match() {
        let processor = BlacklistProcessor::new(vec!["API".to_string()]);
        let (protected, map) = processor.protect("Hello World");
        assert_eq!(protected, "Hello World");
        assert!(map.is_empty());
    }

    #[test]
    fn test_protect_multiple_occurrences() {
        let processor = BlacklistProcessor::new(vec!["API".to_string()]);
        let (protected, map) = processor.protect("API call and API response");
        assert_eq!(
            protected,
            "__BLACKLIST_0__ call and __BLACKLIST_0__ response"
        );
        assert_eq!(map.len(), 1);
        assert_eq!(map["__BLACKLIST_0__"], "API");
    }

    #[test]
    fn test_protect_skips_empty_blacklist_words() {
        let processor =
            BlacklistProcessor::new(vec!["".to_string(), "API".to_string(), "".to_string()]);
        let (protected, map) = processor.protect("Use API");
        // Empty strings are skipped, index 1 is "API"
        assert!(protected.contains("__BLACKLIST_1__"));
        assert!(!protected.contains("API"));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_protect_special_characters() {
        let processor = BlacklistProcessor::new(vec!["C++".to_string(), "C#".to_string()]);
        let (protected, map) = processor.protect("Learn C++ and C#");
        assert!(!protected.contains("C++"));
        assert!(!protected.contains("C#"));
        assert!(protected.contains("__BLACKLIST_0__"));
        assert!(protected.contains("__BLACKLIST_1__"));
        assert_eq!(map["__BLACKLIST_0__"], "C++");
        assert_eq!(map["__BLACKLIST_1__"], "C#");
    }

    #[test]
    fn test_protect_unicode_words() {
        let processor = BlacklistProcessor::new(vec!["东京".to_string()]);
        let (protected, map) = processor.protect("去东京旅游");
        assert_eq!(protected, "去__BLACKLIST_0__旅游");
        assert_eq!(map["__BLACKLIST_0__"], "东京");
    }

    #[test]
    fn test_restore_empty_map() {
        let processor = BlacklistProcessor::new(vec!["API".to_string()]);
        let map = HashMap::new();
        let restored = processor.restore("Hello World", &map);
        assert_eq!(restored, "Hello World");
    }

    #[test]
    fn test_restore_case_insensitive() {
        let processor = BlacklistProcessor::new(vec!["API".to_string()]);
        let mut map = HashMap::new();
        map.insert("__BLACKLIST_0__".to_string(), "API".to_string());

        // LLM changed placeholder to lowercase
        let restored = processor.restore("__blacklist_0__ call", &map);
        assert_eq!(restored, "API call");
    }

    #[test]
    fn test_restore_preserves_original_case() {
        let processor = BlacklistProcessor::new(vec!["JavaScript".to_string()]);
        let mut map = HashMap::new();
        map.insert("__BLACKLIST_0__".to_string(), "JavaScript".to_string());

        let restored = processor.restore("使用__BLACKLIST_0__编写", &map);
        assert_eq!(restored, "使用JavaScript编写");
    }

    #[test]
    fn test_protect_and_restore_full_roundtrip() {
        let blacklist = vec!["OpenAI".to_string(), "GPT-4".to_string(), "API".to_string()];
        let processor = BlacklistProcessor::new(blacklist);

        let text = "Call OpenAI GPT-4 API endpoint";
        let (protected, map) = processor.protect(text);

        // All terms replaced
        assert!(!protected.contains("OpenAI"));
        assert!(!protected.contains("GPT-4"));
        assert!(!protected.contains("API"));

        // Restore should recover original
        let restored = processor.restore(&protected, &map);
        assert_eq!(restored, text);
    }

    #[test]
    fn test_is_blacklisted_empty_word() {
        let processor = BlacklistProcessor::new(vec!["API".to_string()]);
        assert!(!processor.is_blacklisted(""));
    }

    #[test]
    fn test_is_blacklisted_empty_blacklist() {
        let processor = BlacklistProcessor::new(vec![]);
        assert!(!processor.is_blacklisted("API"));
    }

    #[test]
    fn test_protect_overlapping_terms() {
        // When one blacklist term is a substring of another
        let processor = BlacklistProcessor::new(vec!["Java".to_string(), "JavaScript".to_string()]);
        let (protected, map) = processor.protect("Use JavaScript and Java");
        // "Java" is replaced first (index 0), which changes "JavaScript" too
        assert!(!protected.contains("Java "));
        // After both replacements, verify map is correct
        assert!(map.contains_key("__BLACKLIST_0__"));
        assert!(map.contains_key("__BLACKLIST_1__"));
    }

    #[test]
    fn test_protect_whitespace_only_text() {
        let processor = BlacklistProcessor::new(vec!["API".to_string()]);
        let (protected, map) = processor.protect("   \n\t  ");
        assert_eq!(protected, "   \n\t  ");
        assert!(map.is_empty());
    }
}

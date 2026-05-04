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

    /// Replace blacklisted words with numbered placeholders
    /// Returns (protected_text, placeholder_map)
    pub fn protect(&self, text: &str) -> (String, HashMap<String, String>) {
        let mut protected_text = text.to_string();
        let mut placeholder_map = HashMap::new();

        for (i, word) in self.blacklist.iter().enumerate() {
            if word.is_empty() {
                continue;
            }

            let placeholder = format!("__BLACKLIST_{}__", i);

            if protected_text.contains(word) {
                placeholder_map.insert(placeholder.clone(), word.clone());
                protected_text = protected_text.replace(word, &placeholder);
            }
        }

        (protected_text, placeholder_map)
    }

    /// Restore blacklisted words from placeholders
    pub fn restore(&self, text: &str, placeholder_map: &HashMap<String, String>) -> String {
        let mut restored_text = text.to_string();

        for (placeholder, original) in placeholder_map {
            // Case-insensitive restore (LLM might change case)
            let lower_placeholder = placeholder.to_lowercase();
            let lower_text = restored_text.to_lowercase();

            if let Some(pos) = lower_text.find(&lower_placeholder) {
                let actual_placeholder = &restored_text[pos..pos + placeholder.len()];
                restored_text = restored_text.replacen(actual_placeholder, original, 1);
            }
        }

        restored_text
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
}

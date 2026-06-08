use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// Re-export shared type from models
pub use crate::models::glossary::GlossaryEntry;

#[derive(Debug, Serialize, Deserialize)]
pub struct Glossary {
    entries: HashMap<String, Vec<GlossaryEntry>>,
    path: PathBuf,
}

impl Glossary {
    pub async fn load() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("moontranslator")
            .join("glossary.json");

        let entries = if tokio::fs::metadata(&path).await.is_ok() {
            let data = tokio::fs::read_to_string(&path).await.unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };

        Glossary { entries, path }
    }

    pub async fn save(&self) {
        if let Some(parent) = self.path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                tracing::warn!("Failed to create glossary directory {:?}: {}", parent, e);
            }
        }
        match serde_json::to_string_pretty(&self.entries) {
            Ok(data) => {
                if let Err(e) = tokio::fs::write(&self.path, data).await {
                    tracing::error!("Failed to save glossary to {:?}: {}", self.path, e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize glossary: {}", e);
            }
        }
    }

    pub async fn add_entry(&mut self, lang_pair: String, entry: GlossaryEntry) {
        self.entries
            .entry(lang_pair)
            .or_insert_with(Vec::new)
            .push(entry);
        self.save().await;
    }

    pub async fn remove_entry(&mut self, lang_pair: &str, source: &str) -> bool {
        if let Some(entries) = self.entries.get_mut(lang_pair) {
            let len_before = entries.len();
            entries.retain(|e| e.source != source);
            if entries.len() < len_before {
                self.save().await;
                return true;
            }
        }
        false
    }

    pub fn get_entries(&self, lang_pair: &str) -> Vec<GlossaryEntry> {
        self.entries.get(lang_pair).cloned().unwrap_or_default()
    }

    pub fn get_all_entries(&self) -> &HashMap<String, Vec<GlossaryEntry>> {
        &self.entries
    }

    pub fn apply_glossary(&self, text: &mut String, lang_pair: &str) {
        if let Some(entries) = self.entries.get(lang_pair) {
            for entry in entries {
                *text = text.replace(&entry.source, &entry.target);
            }
        }
    }

    /// Format glossary entries as a hint string for LLM system prompt injection.
    /// Returns empty string if no entries exist for the language pair.
    pub fn format_hint(&self, lang_pair: &str) -> String {
        let entries = match self.entries.get(lang_pair) {
            Some(e) if !e.is_empty() => e,
            _ => return String::new(),
        };

        let mut lines = Vec::new();
        lines.push("术语表（翻译时必须使用以下译法）：".to_string());
        for entry in entries {
            if let Some(ref ctx) = entry.context {
                lines.push(format!("{} → {} ({})", entry.source, entry.target, ctx));
            } else {
                lines.push(format!("{} → {}", entry.source, entry.target));
            }
        }
        lines.join("\n")
    }

    /// Create a Glossary for testing with pre-populated entries.
    #[cfg(test)]
    fn test_fixture() -> Self {
        let mut entries = HashMap::new();
        entries.insert(
            "ja-zh".to_string(),
            vec![
                GlossaryEntry {
                    source: "自動翻訳".to_string(),
                    target: "自动翻译".to_string(),
                    context: None,
                },
                GlossaryEntry {
                    source: "機械学習".to_string(),
                    target: "机器学习".to_string(),
                    context: Some("ML domain".to_string()),
                },
            ],
        );
        Glossary {
            entries,
            path: PathBuf::from("/tmp/test_glossary.json"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_glossary_replaces_terms() {
        let glossary = Glossary::test_fixture();
        let mut text = "自動翻訳と機械学習".to_string();
        glossary.apply_glossary(&mut text, "ja-zh");
        assert_eq!(text, "自动翻译と机器学习");
    }

    #[test]
    fn test_apply_glossary_no_match() {
        let glossary = Glossary::test_fixture();
        let mut text = "こんにちは".to_string();
        glossary.apply_glossary(&mut text, "ja-zh");
        assert_eq!(text, "こんにちは");
    }

    #[test]
    fn test_apply_glossary_wrong_lang_pair() {
        let glossary = Glossary::test_fixture();
        let mut text = "自動翻訳".to_string();
        glossary.apply_glossary(&mut text, "en-zh");
        assert_eq!(text, "自動翻訳");
    }

    #[test]
    fn test_format_hint_with_entries() {
        let glossary = Glossary::test_fixture();
        let hint = glossary.format_hint("ja-zh");
        assert!(hint.contains("术语表"));
        assert!(hint.contains("自動翻訳 → 自动翻译"));
        assert!(hint.contains("機械学習 → 机器学习 (ML domain)"));
    }

    #[test]
    fn test_format_hint_empty_lang_pair() {
        let glossary = Glossary::test_fixture();
        let hint = glossary.format_hint("en-fr");
        assert!(hint.is_empty());
    }

    #[test]
    fn test_get_entries_returns_cloned_vec() {
        let glossary = Glossary::test_fixture();
        let entries = glossary.get_entries("ja-zh");
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_get_entries_missing_pair() {
        let glossary = Glossary::test_fixture();
        let entries = glossary.get_entries("en-fr");
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_add_entry() {
        let mut glossary = Glossary::test_fixture();
        // Use a temp path that we won't actually write to
        glossary.path = PathBuf::from("/dev/null/test_add");
        glossary.add_entry(
            "en-zh".to_string(),
            GlossaryEntry {
                source: "hello".to_string(),
                target: "你好".to_string(),
                context: None,
            },
        ).await;
        let entries = glossary.get_entries("en-zh");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "hello");
    }

    #[tokio::test]
    async fn test_remove_entry() {
        let mut glossary = Glossary::test_fixture();
        glossary.path = PathBuf::from("/dev/null/test_remove");
        let removed = glossary.remove_entry("ja-zh", "自動翻訳").await;
        assert!(removed);
        let entries = glossary.get_entries("ja-zh");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "機械学習");
    }

    #[tokio::test]
    async fn test_remove_entry_not_found() {
        let mut glossary = Glossary::test_fixture();
        glossary.path = PathBuf::from("/dev/null/test_remove_nf");
        let removed = glossary.remove_entry("ja-zh", "存在しない").await;
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_remove_entry_wrong_lang_pair() {
        let mut glossary = Glossary::test_fixture();
        glossary.path = PathBuf::from("/dev/null/test_remove_wlp");
        let removed = glossary.remove_entry("en-fr", "自動翻訳").await;
        assert!(!removed);
        // Original entries should be untouched
        assert_eq!(glossary.get_entries("ja-zh").len(), 2);
    }

    #[test]
    fn test_get_all_entries() {
        let glossary = Glossary::test_fixture();
        let all = glossary.get_all_entries();
        assert!(all.contains_key("ja-zh"));
        assert_eq!(all.len(), 1);
        assert_eq!(all["ja-zh"].len(), 2);
    }

    #[test]
    fn test_get_all_entries_empty() {
        let glossary = Glossary {
            entries: HashMap::new(),
            path: PathBuf::from("/tmp/empty.json"),
        };
        assert!(glossary.get_all_entries().is_empty());
    }

    #[test]
    fn test_apply_glossary_empty_text() {
        let glossary = Glossary::test_fixture();
        let mut text = String::new();
        glossary.apply_glossary(&mut text, "ja-zh");
        assert!(text.is_empty());
    }

    #[test]
    fn test_apply_glossary_multiple_occurrences() {
        let mut entries = HashMap::new();
        entries.insert(
            "en-zh".to_string(),
            vec![GlossaryEntry {
                source: "hello".to_string(),
                target: "你好".to_string(),
                context: None,
            }],
        );
        let glossary = Glossary {
            entries,
            path: PathBuf::from("/tmp/test_multi.json"),
        };
        let mut text = "hello world hello again hello".to_string();
        glossary.apply_glossary(&mut text, "en-zh");
        assert_eq!(text, "你好 world 你好 again 你好");
    }

    #[test]
    fn test_apply_glossary_partial_match() {
        let mut entries = HashMap::new();
        entries.insert(
            "en-zh".to_string(),
            vec![GlossaryEntry {
                source: "cat".to_string(),
                target: "猫".to_string(),
                context: None,
            }],
        );
        let glossary = Glossary {
            entries,
            path: PathBuf::from("/tmp/test_partial.json"),
        };
        // "cat" is a substring of "category" - this is expected behavior (simple replace)
        let mut text = "category".to_string();
        glossary.apply_glossary(&mut text, "en-zh");
        assert_eq!(text, "猫egory");
    }

    #[test]
    fn test_format_hint_without_context() {
        let mut entries = HashMap::new();
        entries.insert(
            "en-zh".to_string(),
            vec![GlossaryEntry {
                source: "API".to_string(),
                target: "接口".to_string(),
                context: None,
            }],
        );
        let glossary = Glossary {
            entries,
            path: PathBuf::from("/tmp/test_hint.json"),
        };
        let hint = glossary.format_hint("en-zh");
        assert!(hint.contains("术语表"));
        assert!(hint.contains("API → 接口"));
        // Should NOT have parentheses for context
        assert!(!hint.contains("("));
    }

    #[test]
    fn test_format_hint_mixed_context() {
        let glossary = Glossary::test_fixture();
        let hint = glossary.format_hint("ja-zh");
        // First entry has no context
        assert!(hint.contains("自動翻訳 → 自动翻译"));
        // Second entry has context
        assert!(hint.contains("機械学習 → 机器学习 (ML domain)"));
    }

    #[test]
    fn test_format_hint_empty_entries() {
        let mut entries = HashMap::new();
        entries.insert("en-zh".to_string(), vec![]);
        let glossary = Glossary {
            entries,
            path: PathBuf::from("/tmp/test_empty_entries.json"),
        };
        let hint = glossary.format_hint("en-zh");
        assert!(hint.is_empty());
    }

    #[tokio::test]
    async fn test_add_entry_to_existing_lang_pair() {
        let mut glossary = Glossary::test_fixture();
        glossary.path = PathBuf::from("/dev/null/test_add_existing");
        assert_eq!(glossary.get_entries("ja-zh").len(), 2);

        glossary.add_entry(
            "ja-zh".to_string(),
            GlossaryEntry {
                source: "新しい".to_string(),
                target: "新的".to_string(),
                context: None,
            },
        ).await;
        let entries = glossary.get_entries("ja-zh");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[2].source, "新しい");
    }

    #[test]
    fn test_glossary_entry_serde() {
        let entry = GlossaryEntry {
            source: "test".to_string(),
            target: "测试".to_string(),
            context: Some("unit test".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: GlossaryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source, "test");
        assert_eq!(deserialized.target, "测试");
        assert_eq!(deserialized.context, Some("unit test".to_string()));
    }

    #[test]
    fn test_glossary_entry_without_context_serde() {
        let entry = GlossaryEntry {
            source: "hello".to_string(),
            target: "你好".to_string(),
            context: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: GlossaryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source, "hello");
        assert_eq!(deserialized.target, "你好");
        assert!(deserialized.context.is_none());
    }
}

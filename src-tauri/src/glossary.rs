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
    pub fn load() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("moontranslator")
            .join("glossary.json");

        let entries = if path.exists() {
            let data = std::fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };

        Glossary { entries, path }
    }

    pub fn save(&self) {
        if let Some(parent) = self.path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create glossary directory {:?}: {}", parent, e);
            }
        }
        match serde_json::to_string_pretty(&self.entries) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&self.path, data) {
                    tracing::error!("Failed to save glossary to {:?}: {}", self.path, e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize glossary: {}", e);
            }
        }
    }

    pub fn add_entry(&mut self, lang_pair: String, entry: GlossaryEntry) {
        self.entries
            .entry(lang_pair)
            .or_insert_with(Vec::new)
            .push(entry);
        self.save();
    }

    pub fn remove_entry(&mut self, lang_pair: &str, source: &str) -> bool {
        if let Some(entries) = self.entries.get_mut(lang_pair) {
            let len_before = entries.len();
            entries.retain(|e| e.source != source);
            if entries.len() < len_before {
                self.save();
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

    #[test]
    fn test_add_entry() {
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
        );
        let entries = glossary.get_entries("en-zh");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "hello");
    }

    #[test]
    fn test_remove_entry() {
        let mut glossary = Glossary::test_fixture();
        glossary.path = PathBuf::from("/dev/null/test_remove");
        let removed = glossary.remove_entry("ja-zh", "自動翻訳");
        assert!(removed);
        let entries = glossary.get_entries("ja-zh");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "機械学習");
    }

    #[test]
    fn test_remove_entry_not_found() {
        let mut glossary = Glossary::test_fixture();
        glossary.path = PathBuf::from("/dev/null/test_remove_nf");
        let removed = glossary.remove_entry("ja-zh", "存在しない");
        assert!(!removed);
    }
}

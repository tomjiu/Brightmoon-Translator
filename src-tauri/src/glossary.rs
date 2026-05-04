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
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(&self.entries) {
            let _ = std::fs::write(&self.path, data);
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
        self.entries
            .get(lang_pair)
            .cloned()
            .unwrap_or_default()
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
}

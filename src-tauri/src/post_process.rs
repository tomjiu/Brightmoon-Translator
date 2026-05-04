use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            trim_whitespace: true,
            fix_punctuation: true,
            fix_newlines: true,
        }
    }
}

pub struct PostProcessor {
    config: Mutex<PostProcessConfig>,
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    std::fs::create_dir_all(&path).ok();
    path.push("post_process.json");
    path
}

impl PostProcessor {
    pub fn load() -> Self {
        let path = config_path();
        let config = if path.exists() {
            let data = std::fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            PostProcessConfig::default()
        };

        Self {
            config: Mutex::new(config),
        }
    }

    pub fn save(&self) {
        let config = self.config.lock().unwrap();
        let path = config_path();
        if let Ok(data) = serde_json::to_string_pretty(&*config) {
            std::fs::write(path, data).ok();
        }
    }

    pub fn get_config(&self) -> PostProcessConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn update_config(&self, config: PostProcessConfig) {
        let mut current = self.config.lock().unwrap();
        *current = config;
        drop(current);
        self.save();
    }

    pub fn add_rule(&self, rule: ReplacementRule) {
        let mut config = self.config.lock().unwrap();
        config.rules.push(rule);
        drop(config);
        self.save();
    }

    pub fn remove_rule(&self, id: &str) {
        let mut config = self.config.lock().unwrap();
        config.rules.retain(|r| r.id != id);
        drop(config);
        self.save();
    }

    pub fn update_rule(&self, id: &str, rule: ReplacementRule) {
        let mut config = self.config.lock().unwrap();
        if let Some(existing) = config.rules.iter_mut().find(|r| r.id == id) {
            *existing = rule;
        }
        drop(config);
        self.save();
    }

    pub fn process(&self, text: &str) -> String {
        let config = self.config.lock().unwrap();
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

// Add regex crate to dependencies
pub fn create_default_rules() -> Vec<ReplacementRule> {
    vec![
        ReplacementRule {
            id: "1".to_string(),
            pattern: "您".to_string(),
            replacement: "你".to_string(),
            enabled: false,
            is_regex: false,
        },
    ]
}

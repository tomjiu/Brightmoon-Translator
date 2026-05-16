use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreProcessRule {
    pub id: String,
    pub pattern: String,
    pub replacement: String,
    pub enabled: bool,
    pub is_regex: bool,
    /// Optional: only apply to specific language pairs (e.g., "ja-zh")
    pub lang_pair: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreProcessConfig {
    pub rules: Vec<PreProcessRule>,
    /// Remove leading/trailing whitespace before translation
    pub trim_whitespace: bool,
    /// Normalize unicode characters (e.g., fullwidth → halfwidth)
    pub normalize_unicode: bool,
    /// Remove control characters
    pub remove_control_chars: bool,
}

impl Default for PreProcessConfig {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            trim_whitespace: true,
            normalize_unicode: false,
            remove_control_chars: true,
        }
    }
}

pub struct PreProcessor {
    config: Mutex<PreProcessConfig>,
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    std::fs::create_dir_all(&path).ok();
    path.push("pre_process.json");
    path
}

impl PreProcessor {
    pub fn load() -> Self {
        let path = config_path();
        let config = if path.exists() {
            let data = std::fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            PreProcessConfig::default()
        };

        Self {
            config: Mutex::new(config),
        }
    }

    pub fn save(&self) {
        let config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        let path = config_path();
        if let Ok(data) = serde_json::to_string_pretty(&*config) {
            std::fs::write(path, data).ok();
        }
    }

    pub fn get_config(&self) -> PreProcessConfig {
        self.config.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn update_config(&self, config: PreProcessConfig) {
        let mut current = self.config.lock().unwrap_or_else(|e| e.into_inner());
        *current = config;
        drop(current);
        self.save();
    }

    pub fn add_rule(&self, rule: PreProcessRule) {
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

    pub fn update_rule(&self, id: &str, rule: PreProcessRule) {
        let mut config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = config.rules.iter_mut().find(|r| r.id == id) {
            *existing = rule;
        }
        drop(config);
        self.save();
    }

    /// Process text before translation
    pub fn process(&self, text: &str, lang_pair: Option<&str>) -> String {
        let config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        let mut result = text.to_string();

        // Remove control characters
        if config.remove_control_chars {
            result = remove_control_chars(&result);
        }

        // Normalize unicode
        if config.normalize_unicode {
            result = normalize_unicode(&result);
        }

        // Apply replacement rules
        for rule in &config.rules {
            if !rule.enabled {
                continue;
            }

            // Check language pair filter
            if let (Some(ref rule_lang), Some(current_lang)) = (&rule.lang_pair, lang_pair) {
                if rule_lang != current_lang && rule_lang != "all" {
                    continue;
                }
            }

            if rule.is_regex {
                if let Ok(re) = Regex::new(&rule.pattern) {
                    result = re.replace_all(&result, &rule.replacement).to_string();
                }
            } else {
                result = result.replace(&rule.pattern, &rule.replacement);
            }
        }

        // Trim whitespace
        if config.trim_whitespace {
            result = result.trim().to_string();
        }

        result
    }
}

fn remove_control_chars(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .collect()
}

fn normalize_unicode(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for c in text.chars() {
        let code = c as u32;
        // Fullwidth ASCII (！..～) → halfwidth
        if code >= 0xFF01 && code <= 0xFF5E {
            result.push(char::from_u32(code - 0xFF01 + 0x21).unwrap_or(c));
        }
        // Fullwidth space → halfwidth space
        else if code == 0x3000 {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

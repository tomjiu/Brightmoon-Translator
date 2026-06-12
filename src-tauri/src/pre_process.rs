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
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!("Failed to create config directory {:?}: {}", path, e);
    }
    path.push("pre_process.json");
    path
}

impl PreProcessor {
    pub fn load() -> Self {
        let path = config_path();
        let config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
                    tracing::error!("Failed to parse pre-process config {:?}: {}", path, e);
                    PreProcessConfig::default()
                }),
                Err(e) => {
                    tracing::error!("Failed to read pre-process config {:?}: {}", path, e);
                    PreProcessConfig::default()
                },
            }
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
        match serde_json::to_string_pretty(&*config) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    tracing::error!("Failed to save pre-process config {:?}: {}", path, e);
                }
            },
            Err(e) => {
                tracing::error!("Failed to serialize pre-process config: {}", e);
            },
        }
    }

    pub fn get_config(&self) -> PreProcessConfig {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_control_chars_preserves_newlines_and_tabs() {
        let input = "hello\nworld\r\ttab";
        assert_eq!(remove_control_chars(input), "hello\nworld\r\ttab");
    }

    #[test]
    fn test_remove_control_chars_removes_null_and_bell() {
        let input = "hello\x00world\x07!";
        assert_eq!(remove_control_chars(input), "helloworld!");
    }

    #[test]
    fn test_normalize_unicode_fullwidth_ascii() {
        // Ａ is fullwidth A (U+FF21), ！ is fullwidth ! (U+FF01)
        let input = "Ｈｅｌｌｏ！";
        assert_eq!(normalize_unicode(input), "Hello!");
    }

    #[test]
    fn test_normalize_unicode_fullwidth_space() {
        let input = "hello　world"; // ideographic space U+3000
        assert_eq!(normalize_unicode(input), "hello world");
    }

    #[test]
    fn test_normalize_unicode_preserves_cjk() {
        let input = "你好世界";
        assert_eq!(normalize_unicode(input), "你好世界");
    }

    #[test]
    fn test_pre_processor_process_trims_whitespace() {
        let processor = PreProcessor {
            config: Mutex::new(PreProcessConfig {
                rules: Vec::new(),
                trim_whitespace: true,
                normalize_unicode: false,
                remove_control_chars: false,
            }),
        };
        assert_eq!(processor.process("  hello  ", None), "hello");
    }

    #[test]
    fn test_pre_processor_process_normalizes_unicode() {
        let processor = PreProcessor {
            config: Mutex::new(PreProcessConfig {
                rules: Vec::new(),
                trim_whitespace: false,
                normalize_unicode: true,
                remove_control_chars: false,
            }),
        };
        assert_eq!(processor.process("ＡＢＣ", None), "ABC");
    }

    #[test]
    fn test_pre_processor_lang_pair_filter() {
        let processor = PreProcessor {
            config: Mutex::new(PreProcessConfig {
                rules: vec![PreProcessRule {
                    id: "1".to_string(),
                    pattern: "foo".to_string(),
                    replacement: "bar".to_string(),
                    enabled: true,
                    is_regex: false,
                    lang_pair: Some("ja-zh".to_string()),
                }],
                trim_whitespace: false,
                normalize_unicode: false,
                remove_control_chars: false,
            }),
        };
        // Should apply when lang_pair matches
        assert_eq!(processor.process("foo", Some("ja-zh")), "bar");
        // Should not apply when lang_pair doesn't match
        assert_eq!(processor.process("foo", Some("en-zh")), "foo");
        // Should apply when rule has no lang_pair filter
    }
}

use super::TranslationEngine;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Offline translation model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineModel {
    pub id: String,
    pub name: String,
    pub source_lang: String,
    pub target_lang: String,
    pub version: String,
    pub size_bytes: u64,
    pub downloaded: bool,
    pub download_url: String,
}

/// Translation dictionary entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub context: Option<String>,
}

/// Language model containing phrase dictionary and rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageModel {
    pub source_lang: String,
    pub target_lang: String,
    pub version: String,
    pub phrases: Vec<DictionaryEntry>,
    #[serde(default)]
    pub common_words: HashMap<String, String>,
}

/// Offline translation engine using local phrase dictionaries
pub struct OfflineEngine {
    models: Arc<RwLock<HashMap<String, LanguageModel>>>,
    model_dir: PathBuf,
}

impl OfflineEngine {
    pub fn new(model_dir: Option<&str>) -> Self {
        let dir = model_dir.map(PathBuf::from).unwrap_or_else(|| {
            let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
            path.push("moontranslator");
            path.push("offline_models");
            path
        });

        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            model_dir: dir,
        }
    }

    /// Get the model directory path
    pub fn model_dir(&self) -> &PathBuf {
        &self.model_dir
    }

    /// Load a language model from disk
    pub async fn load_model(&self, source: &str, target: &str) -> anyhow::Result<()> {
        let model_id = format!("{}-{}", source, target);
        let model_path = self.model_dir.join(format!("{}.json", model_id));

        if !model_path.exists() {
            return Err(anyhow::anyhow!(
                "Model file not found: {}",
                model_path.display()
            ));
        }

        let content = tokio::fs::read_to_string(&model_path).await?;
        let model: LanguageModel = serde_json::from_str(&content)?;

        let mut models = self.models.write().await;
        models.insert(model_id, model);

        tracing::info!("[OfflineEngine] Loaded model: {}-{}", source, target);
        Ok(())
    }

    /// Load all downloaded models from the model directory
    pub async fn load_all_models(&self) -> anyhow::Result<Vec<String>> {
        if !self.model_dir.exists() {
            tokio::fs::create_dir_all(&self.model_dir).await?;
            return Ok(Vec::new());
        }

        let mut loaded = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.model_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let parts: Vec<&str> = stem.split('-').collect();
                    if parts.len() == 2 {
                        match self.load_model(parts[0], parts[1]).await {
                            Ok(()) => loaded.push(stem.to_string()),
                            Err(e) => {
                                tracing::warn!(
                                    "[OfflineEngine] Failed to load model {}: {}",
                                    stem,
                                    e
                                );
                            },
                        }
                    }
                }
            }
        }

        tracing::info!("[OfflineEngine] Loaded {} models", loaded.len());
        Ok(loaded)
    }

    /// Get list of available language pairs
    pub async fn available_pairs(&self) -> Vec<String> {
        let models = self.models.read().await;
        models.keys().cloned().collect()
    }

    /// Check if a model is downloaded
    pub fn is_model_downloaded(&self, source: &str, target: &str) -> bool {
        let model_id = format!("{}-{}", source, target);
        self.model_dir.join(format!("{}.json", model_id)).exists()
    }

    /// Get list of all available models (including not downloaded)
    pub fn available_models() -> Vec<OfflineModel> {
        vec![
            OfflineModel {
                id: "en-zh".to_string(),
                name: "English to Chinese".to_string(),
                source_lang: "en".to_string(),
                target_lang: "zh".to_string(),
                version: "1.0.0".to_string(),
                size_bytes: 2_500_000, // ~2.5MB
                downloaded: false,
                download_url: "https://github.com/moon-translator/offline-models/releases/download/v1.0.0/en-zh.json".to_string(),
            },
            OfflineModel {
                id: "zh-en".to_string(),
                name: "Chinese to English".to_string(),
                source_lang: "zh".to_string(),
                target_lang: "en".to_string(),
                version: "1.0.0".to_string(),
                size_bytes: 2_800_000, // ~2.8MB
                downloaded: false,
                download_url: "https://github.com/moon-translator/offline-models/releases/download/v1.0.0/zh-en.json".to_string(),
            },
            OfflineModel {
                id: "ja-zh".to_string(),
                name: "Japanese to Chinese".to_string(),
                source_lang: "ja".to_string(),
                target_lang: "zh".to_string(),
                version: "1.0.0".to_string(),
                size_bytes: 3_000_000, // ~3MB
                downloaded: false,
                download_url: "https://github.com/moon-translator/offline-models/releases/download/v1.0.0/ja-zh.json".to_string(),
            },
            OfflineModel {
                id: "en-ja".to_string(),
                name: "English to Japanese".to_string(),
                source_lang: "en".to_string(),
                target_lang: "ja".to_string(),
                version: "1.0.0".to_string(),
                size_bytes: 2_200_000, // ~2.2MB
                downloaded: false,
                download_url: "https://github.com/moon-translator/offline-models/releases/download/v1.0.0/en-ja.json".to_string(),
            },
            OfflineModel {
                id: "ko-zh".to_string(),
                name: "Korean to Chinese".to_string(),
                source_lang: "ko".to_string(),
                target_lang: "zh".to_string(),
                version: "1.0.0".to_string(),
                size_bytes: 2_600_000, // ~2.6MB
                downloaded: false,
                download_url: "https://github.com/moon-translator/offline-models/releases/download/v1.0.0/ko-zh.json".to_string(),
            },
        ]
    }

    /// Download a model from the remote URL
    pub async fn download_model(&self, model_id: &str) -> anyhow::Result<()> {
        let models = Self::available_models();
        let model = models
            .iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_id))?;

        // Create model directory if it doesn't exist
        if !self.model_dir.exists() {
            tokio::fs::create_dir_all(&self.model_dir).await?;
        }

        let model_path = self.model_dir.join(format!("{}.json", model_id));

        // Download the model file
        tracing::info!(
            "[OfflineEngine] Downloading model {} from {}",
            model_id,
            model.download_url
        );

        let client = reqwest::Client::new();
        let response = client.get(&model.download_url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to download model: HTTP {}",
                response.status()
            ));
        }

        let content = response.bytes().await?;
        tokio::fs::write(&model_path, &content).await?;

        // Verify the downloaded file is valid JSON
        let _: LanguageModel = serde_json::from_slice(&content)?;

        tracing::info!(
            "[OfflineEngine] Successfully downloaded model: {}",
            model_id
        );
        Ok(())
    }

    /// Delete a downloaded model
    pub async fn delete_model(&self, source: &str, target: &str) -> anyhow::Result<()> {
        let model_id = format!("{}-{}", source, target);
        let model_path = self.model_dir.join(format!("{}.json", model_id));

        if model_path.exists() {
            tokio::fs::remove_file(&model_path).await?;

            // Remove from loaded models
            let mut models = self.models.write().await;
            models.remove(&model_id);

            tracing::info!("[OfflineEngine] Deleted model: {}", model_id);
        }

        Ok(())
    }

    /// Get model file size
    pub fn model_size(&self, source: &str, target: &str) -> Option<u64> {
        let model_id = format!("{}-{}", source, target);
        let model_path = self.model_dir.join(format!("{}.json", model_id));

        if model_path.exists() {
            std::fs::metadata(&model_path).ok().map(|m| m.len())
        } else {
            None
        }
    }

    /// Simple word-by-word translation using the model
    fn translate_with_model(model: &LanguageModel, text: &str) -> String {
        let text_lower = text.to_lowercase();

        // First try exact phrase match
        for entry in &model.phrases {
            if text_lower == entry.source.to_lowercase() {
                return entry.target.clone();
            }
        }

        // Try word-by-word translation
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut translated_words = Vec::new();

        for word in words {
            let word_lower = word.to_lowercase();

            // Check common words dictionary first
            if let Some(translation) = model.common_words.get(&word_lower) {
                translated_words.push(translation.clone());
            } else {
                // Try to find in phrases
                let mut found = false;
                for entry in &model.phrases {
                    if entry.source.to_lowercase() == word_lower {
                        translated_words.push(entry.target.clone());
                        found = true;
                        break;
                    }
                }

                if !found {
                    // Keep original word if no translation found
                    translated_words.push(word.to_string());
                }
            }
        }

        translated_words.join(" ")
    }
}

#[async_trait]
impl TranslationEngine for OfflineEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let model_id = format!("{}-{}", from, to);
        let models = self.models.read().await;

        if let Some(model) = models.get(&model_id) {
            let result = Self::translate_with_model(model, text);

            if result.is_empty() || result == text {
                // If translation is same as source or empty, try reverse lookup
                return Err(anyhow::anyhow!("No translation found for the given text"));
            }

            Ok(result)
        } else {
            Err(anyhow::anyhow!(
                "Offline model not available for {} -> {}. Please download the model first.",
                from,
                to
            ))
        }
    }

    fn name(&self) -> &str {
        "Offline"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Generate a sample model for testing
pub fn generate_sample_model(source: &str, target: &str) -> LanguageModel {
    let mut phrases = Vec::new();
    let mut common_words = HashMap::new();

    // Add some basic phrases based on language pair
    match (source, target) {
        ("en", "zh") => {
            phrases.push(DictionaryEntry {
                source: "hello".to_string(),
                target: "你好".to_string(),
                context: Some("greeting".to_string()),
            });
            phrases.push(DictionaryEntry {
                source: "goodbye".to_string(),
                target: "再见".to_string(),
                context: Some("farewell".to_string()),
            });
            phrases.push(DictionaryEntry {
                source: "thank you".to_string(),
                target: "谢谢".to_string(),
                context: Some("gratitude".to_string()),
            });
            phrases.push(DictionaryEntry {
                source: "please".to_string(),
                target: "请".to_string(),
                context: Some("polite".to_string()),
            });
            phrases.push(DictionaryEntry {
                source: "yes".to_string(),
                target: "是".to_string(),
                context: None,
            });
            phrases.push(DictionaryEntry {
                source: "no".to_string(),
                target: "不".to_string(),
                context: None,
            });

            common_words.insert("i".to_string(), "我".to_string());
            common_words.insert("you".to_string(), "你".to_string());
            common_words.insert("he".to_string(), "他".to_string());
            common_words.insert("she".to_string(), "她".to_string());
            common_words.insert("we".to_string(), "我们".to_string());
            common_words.insert("they".to_string(), "他们".to_string());
            common_words.insert("is".to_string(), "是".to_string());
            common_words.insert("are".to_string(), "是".to_string());
            common_words.insert("am".to_string(), "是".to_string());
            common_words.insert("good".to_string(), "好".to_string());
            common_words.insert("bad".to_string(), "坏".to_string());
            common_words.insert("big".to_string(), "大".to_string());
            common_words.insert("small".to_string(), "小".to_string());
            common_words.insert("water".to_string(), "水".to_string());
            common_words.insert("food".to_string(), "食物".to_string());
            common_words.insert("help".to_string(), "帮助".to_string());
            common_words.insert("time".to_string(), "时间".to_string());
            common_words.insert("day".to_string(), "天".to_string());
            common_words.insert("night".to_string(), "夜".to_string());
        },
        ("zh", "en") => {
            phrases.push(DictionaryEntry {
                source: "你好".to_string(),
                target: "hello".to_string(),
                context: Some("greeting".to_string()),
            });
            phrases.push(DictionaryEntry {
                source: "再见".to_string(),
                target: "goodbye".to_string(),
                context: Some("farewell".to_string()),
            });
            phrases.push(DictionaryEntry {
                source: "谢谢".to_string(),
                target: "thank you".to_string(),
                context: Some("gratitude".to_string()),
            });
            phrases.push(DictionaryEntry {
                source: "请".to_string(),
                target: "please".to_string(),
                context: Some("polite".to_string()),
            });
            phrases.push(DictionaryEntry {
                source: "是".to_string(),
                target: "yes".to_string(),
                context: None,
            });
            phrases.push(DictionaryEntry {
                source: "不是".to_string(),
                target: "no".to_string(),
                context: None,
            });

            common_words.insert("我".to_string(), "I".to_string());
            common_words.insert("你".to_string(), "you".to_string());
            common_words.insert("他".to_string(), "he".to_string());
            common_words.insert("她".to_string(), "she".to_string());
            common_words.insert("我们".to_string(), "we".to_string());
            common_words.insert("他们".to_string(), "they".to_string());
            common_words.insert("好".to_string(), "good".to_string());
            common_words.insert("坏".to_string(), "bad".to_string());
            common_words.insert("大".to_string(), "big".to_string());
            common_words.insert("小".to_string(), "small".to_string());
            common_words.insert("水".to_string(), "water".to_string());
            common_words.insert("帮助".to_string(), "help".to_string());
            common_words.insert("时间".to_string(), "time".to_string());
        },
        _ => {
            // Generic placeholder for other language pairs
            phrases.push(DictionaryEntry {
                source: "hello".to_string(),
                target: "hello".to_string(),
                context: None,
            });
        },
    }

    LanguageModel {
        source_lang: source.to_string(),
        target_lang: target.to_string(),
        version: "1.0.0".to_string(),
        phrases,
        common_words,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_en_to_zh() {
        let model = generate_sample_model("en", "zh");
        let result = OfflineEngine::translate_with_model(&model, "hello");
        assert_eq!(result, "你好");
    }

    #[test]
    fn test_translate_zh_to_en() {
        let model = generate_sample_model("zh", "en");
        let result = OfflineEngine::translate_with_model(&model, "你好");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_word_by_word_translation() {
        let model = generate_sample_model("en", "zh");
        let result = OfflineEngine::translate_with_model(&model, "I am good");
        assert_eq!(result, "我 是 好");
    }
}

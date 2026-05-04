use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTranslation {
    pub results: Vec<(String, String)>, // (engine, text)
    pub timestamp: i64,
}

pub struct TranslationCache {
    cache: Arc<RwLock<HashMap<String, CachedTranslation>>>,
    max_size: usize,
}

fn cache_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    std::fs::create_dir_all(&path).ok();
    path.push("cache.json");
    path
}

impl TranslationCache {
    pub fn new(max_size: usize) -> Self {
        let cache_data = if cache_path().exists() {
            let data = std::fs::read_to_string(cache_path()).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };

        Self {
            cache: Arc::new(RwLock::new(cache_data)),
            max_size,
        }
    }

    fn make_key(text: &str, from: &str, to: &str) -> String {
        format!("{}|{}|{}", from, to, text)
    }

    pub async fn get(&self, text: &str, from: &str, to: &str) -> Option<CachedTranslation> {
        let key = Self::make_key(text, from, to);
        let cache = self.cache.read().await;
        cache.get(&key).cloned()
    }

    pub async fn set(&self, text: &str, from: &str, to: &str, results: Vec<(String, String)>) {
        let key = Self::make_key(text, from, to);
        let entry = CachedTranslation {
            results,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let mut cache = self.cache.write().await;

        // Evict oldest entries if cache is full
        if cache.len() >= self.max_size {
            if let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, v)| v.timestamp)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }

        cache.insert(key, entry);

        // Persist to disk
        self.save_to_disk(&cache).await;
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        self.save_to_disk(&cache).await;
    }

    pub async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    async fn save_to_disk(&self, cache: &HashMap<String, CachedTranslation>) {
        let path = cache_path();
        if let Ok(data) = serde_json::to_string_pretty(cache) {
            let _ = tokio::fs::write(path, data).await;
        }
    }
}

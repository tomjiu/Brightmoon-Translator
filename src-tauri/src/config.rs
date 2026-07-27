// Re-export all config types from the shared models module
pub use crate::models::config::*;

use crate::security::{decrypt_secret, encrypt_secret};
use std::path::PathBuf;

/// Platform-specific: get the config file path
fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!("Failed to create config directory {:?}: {}", path, e);
    }
    path.push("config.json");
    path
}

impl AppConfig {
    /// Load config from platform-specific config directory
    pub fn load() -> Self {
        let path = config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(data) => {
                    match serde_json::from_str::<AppConfig>(&data) {
                        Ok(config) => {
                            let mut config = config;
                            config.decrypt_secrets();
                            return config;
                        },
                        Err(e) => {
                            tracing::error!("Failed to parse config file (using defaults): {}", e);
                            tracing::info!("Config file path: {}", path.display());
                            // Backup the corrupted file
                            let backup = path.with_extension("json.bak");
                            if let Err(bak_err) = std::fs::copy(&path, &backup) {
                                tracing::warn!("Failed to backup corrupted config: {}", bak_err);
                            } else {
                                tracing::info!(
                                    "Corrupted config backed up to: {}",
                                    backup.display()
                                );
                            }
                        },
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read config file: {}", e);
                },
            }
        } else {
            tracing::info!("No config file found, creating default");
        }

        let config = Self::default();
        config.save();
        config
    }

    /// Save config to platform-specific config directory.
    /// Encrypts sensitive fields before writing to disk.
    pub fn save(&self) {
        let mut encrypted = self.clone();
        encrypted.encrypt_secrets();

        let path = config_path();
        match serde_json::to_string_pretty(&encrypted) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    tracing::error!("Failed to save config: {}", e);
                }
            },
            Err(e) => {
                tracing::error!("Failed to serialize config: {}", e);
            },
        }
    }

    /// Encrypt all sensitive fields in place before saving to disk.
    fn encrypt_secrets(&mut self) {
        self.llm.api_key = encrypt_secret(&self.llm.api_key);
        self.llm.api_keys = self
            .llm
            .api_keys
            .iter()
            .map(|k| encrypt_secret(k))
            .collect();
        self.engines.baidu.app_id = encrypt_secret(&self.engines.baidu.app_id);
        self.engines.baidu.secret = encrypt_secret(&self.engines.baidu.secret);
        self.engines.youdao.ocr_app_key = encrypt_secret(&self.engines.youdao.ocr_app_key);
        self.engines.youdao.ocr_app_secret = encrypt_secret(&self.engines.youdao.ocr_app_secret);
        self.engines.deepl.api_key = encrypt_secret(&self.engines.deepl.api_key);
        if let Some(ref key) = self.engines.deeplx.api_key {
            self.engines.deeplx.api_key = Some(encrypt_secret(key));
        }
        self.engines.caiyun.api_token = encrypt_secret(&self.engines.caiyun.api_token);
        self.proxy.password = encrypt_secret(&self.proxy.password);
        self.sync.password = encrypt_secret(&self.sync.password);
        self.collection.eudic.token = encrypt_secret(&self.collection.eudic.token);
        self.collection.shanbay.credential = encrypt_secret(&self.collection.shanbay.credential);
        self.collection.youdao.cookie = encrypt_secret(&self.collection.youdao.cookie);
        self.collection.maimemo.token = encrypt_secret(&self.collection.maimemo.token);
        self.openai_tts.api_key = encrypt_secret(&self.openai_tts.api_key);
    }

    /// Decrypt all sensitive fields in place after loading from disk.
    fn decrypt_secrets(&mut self) {
        self.llm.api_key = decrypt_secret(&self.llm.api_key);
        self.llm.api_keys = self
            .llm
            .api_keys
            .iter()
            .map(|k| decrypt_secret(k))
            .collect();
        self.engines.baidu.app_id = decrypt_secret(&self.engines.baidu.app_id);
        self.engines.baidu.secret = decrypt_secret(&self.engines.baidu.secret);
        self.engines.youdao.ocr_app_key = decrypt_secret(&self.engines.youdao.ocr_app_key);
        self.engines.youdao.ocr_app_secret = decrypt_secret(&self.engines.youdao.ocr_app_secret);
        self.engines.deepl.api_key = decrypt_secret(&self.engines.deepl.api_key);
        if let Some(ref key) = self.engines.deeplx.api_key {
            self.engines.deeplx.api_key = Some(decrypt_secret(key));
        }
        self.engines.caiyun.api_token = decrypt_secret(&self.engines.caiyun.api_token);
        self.proxy.password = decrypt_secret(&self.proxy.password);
        self.sync.password = decrypt_secret(&self.sync.password);
        self.collection.eudic.token = decrypt_secret(&self.collection.eudic.token);
        self.collection.shanbay.credential = decrypt_secret(&self.collection.shanbay.credential);
        self.collection.youdao.cookie = decrypt_secret(&self.collection.youdao.cookie);
        self.collection.maimemo.token = decrypt_secret(&self.collection.maimemo.token);
        self.openai_tts.api_key = decrypt_secret(&self.openai_tts.api_key);
    }

    /// Create a copy with all API keys masked for safe display/export.
    pub fn masked_copy(&self) -> Self {
        let mut masked = self.clone();
        let mask = |s: &str| -> String {
            if s.is_empty() {
                return String::new();
            }
            crate::security::mask_api_key(s)
        };
        masked.llm.api_key = mask(&masked.llm.api_key);
        masked.llm.api_keys = masked.llm.api_keys.iter().map(|k| mask(k)).collect();
        masked.engines.baidu.app_id = mask(&masked.engines.baidu.app_id);
        masked.engines.baidu.secret = mask(&masked.engines.baidu.secret);
        masked.engines.youdao.ocr_app_key = mask(&masked.engines.youdao.ocr_app_key);
        masked.engines.youdao.ocr_app_secret = mask(&masked.engines.youdao.ocr_app_secret);
        masked.engines.deepl.api_key = mask(&masked.engines.deepl.api_key);
        if let Some(ref key) = masked.engines.deeplx.api_key {
            masked.engines.deeplx.api_key = Some(mask(key));
        }
        masked.engines.caiyun.api_token = mask(&masked.engines.caiyun.api_token);
        masked.proxy.password = mask(&masked.proxy.password);
        masked.sync.password = mask(&masked.sync.password);
        masked.collection.eudic.token = mask(&masked.collection.eudic.token);
        masked.collection.shanbay.credential = mask(&masked.collection.shanbay.credential);
        masked.collection.youdao.cookie = mask(&masked.collection.youdao.cookie);
        masked.collection.maimemo.token = mask(&masked.collection.maimemo.token);
        masked.openai_tts.api_key = mask(&masked.openai_tts.api_key);
        masked
    }
}

impl ProxyConfig {
    /// Platform-specific: create a reqwest ClientBuilder with proxy settings applied
    pub fn to_client_builder(&self) -> reqwest::ClientBuilder {
        let mut builder = reqwest::Client::builder();
        if self.enabled && !self.host.is_empty() {
            let proxy_url = format!("{}://{}:{}", self.proxy_type, self.host, self.port);
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                let proxy = if !self.username.is_empty() {
                    proxy.basic_auth(&self.username, &self.password)
                } else {
                    proxy
                };
                builder = builder.proxy(proxy);
            }
        }
        builder
    }
}

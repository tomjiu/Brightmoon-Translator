// Re-export all config types from the shared models module
pub use crate::models::config::*;

use std::path::PathBuf;

/// Platform-specific: get the config file path
fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    std::fs::create_dir_all(&path).ok();
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
                        Ok(config) => return config,
                        Err(e) => {
                            log::error!("Failed to parse config file (using defaults): {}", e);
                            log::info!("Config file path: {}", path.display());
                            // Backup the corrupted file
                            let backup = path.with_extension("json.bak");
                            if let Err(bak_err) = std::fs::copy(&path, &backup) {
                                log::warn!("Failed to backup corrupted config: {}", bak_err);
                            } else {
                                log::info!("Corrupted config backed up to: {}", backup.display());
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to read config file: {}", e);
                }
            }
        } else {
            log::info!("No config file found, creating default");
        }

        let config = Self::default();
        config.save();
        config
    }

    /// Save config to platform-specific config directory
    pub fn save(&self) {
        let path = config_path();
        match serde_json::to_string_pretty(self) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    log::error!("Failed to save config: {}", e);
                }
            }
            Err(e) => {
                log::error!("Failed to serialize config: {}", e);
            }
        }
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

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
            let data = std::fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            let config = Self::default();
            config.save();
            config
        }
    }

    /// Save config to platform-specific config directory
    pub fn save(&self) {
        let path = config_path();
        if let Ok(data) = serde_json::to_string_pretty(self) {
            std::fs::write(path, data).ok();
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

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(rename = "type")]
    pub plugin_type: PluginType,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub translation: Option<TranslationPluginConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Translation,
    Ocr,
    Tts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPluginConfig {
    /// HTTP endpoint for translation (POST)
    /// Request body: { "text": "...", "from": "...", "to": "..." }
    /// Response body: { "translated": "..." }
    pub endpoint: String,
    /// Supported language pairs, e.g. [["en", "zh"], ["zh", "en"]]
    /// Empty means all languages supported
    #[serde(default)]
    pub supported_languages: Vec<Vec<String>>,
    /// Custom headers to send with requests
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub manifest: PluginManifest,
    pub path: String,
}

fn plugins_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    path.push("plugins");
    std::fs::create_dir_all(&path).ok();
    path
}

/// Scan plugins directory and return all discovered plugins
pub fn scan_plugins() -> Vec<PluginInfo> {
    let dir = plugins_dir();
    let mut plugins = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    if let Ok(data) = std::fs::read_to_string(&manifest_path) {
                        if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&data) {
                            plugins.push(PluginInfo {
                                manifest,
                                path: path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    plugins
}

/// Save plugin enabled/disabled state
pub fn set_plugin_enabled(plugin_name: &str, enabled: bool) -> Result<(), String> {
    let dir = plugins_dir();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let manifest_path = path.join("manifest.json");
            if manifest_path.exists() {
                if let Ok(data) = std::fs::read_to_string(&manifest_path) {
                    if let Ok(mut manifest) = serde_json::from_str::<PluginManifest>(&data) {
                        if manifest.name == plugin_name {
                            manifest.enabled = enabled;
                            if let Ok(json) = serde_json::to_string_pretty(&manifest) {
                                std::fs::write(&manifest_path, json)
                                    .map_err(|e| e.to_string())?;
                            }
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Err(format!("Plugin '{}' not found", plugin_name))
}

/// Call a translation plugin's HTTP endpoint
pub async fn call_translation_plugin(
    config: &TranslationPluginConfig,
    text: &str,
    from: &str,
    to: &str,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut req = client.post(&config.endpoint);

    // Add custom headers
    for (key, value) in &config.headers {
        req = req.header(key, value);
    }

    let body = serde_json::json!({
        "text": text,
        "from": from,
        "to": to,
    });

    let resp = req
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Plugin request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Plugin returned status: {}", resp.status()));
    }

    let result: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse plugin response: {}", e))?;

    result
        .get("translated")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Plugin response missing 'translated' field".to_string())
}

/// Get the plugins directory path (for frontend display)
pub fn get_plugins_dir_path() -> String {
    plugins_dir().to_string_lossy().to_string()
}

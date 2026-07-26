use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Current plugin API version. Plugins declaring a higher `minApiVersion` will be rejected.
const PLUGIN_API_VERSION: u32 = 1;

/// Permissions that a plugin can declare in its manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PluginPermission {
    /// Plugin can make outbound HTTP requests.
    Network,
    /// Plugin can read files from its own directory.
    FileRead,
    /// Plugin can write files to its own directory.
    FileWrite,
    /// Plugin can access the system clipboard.
    Clipboard,
    /// Plugin can spawn child processes.
    Process,
    /// Plugin can access translation history.
    History,
    /// Plugin can use OCR capabilities.
    Ocr,
    /// Plugin can use TTS capabilities.
    Tts,
}

/// Resource limits for a sandboxed plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSandboxConfig {
    /// Maximum memory usage in megabytes. Default: 256
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u32,
    /// Maximum CPU percentage (0-100). Default: 50
    #[serde(default = "default_max_cpu_percent")]
    pub max_cpu_percent: u32,
    /// Maximum number of concurrent network connections. Default: 10
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Whether the plugin runs in a sandboxed subprocess. Default: true
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum subprocess restarts before giving up. Default: 3
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
}

fn default_max_memory_mb() -> u32 {
    256
}
fn default_max_cpu_percent() -> u32 {
    50
}
fn default_max_connections() -> u32 {
    10
}
fn default_true() -> bool {
    true
}
fn default_max_restarts() -> u32 {
    3
}

impl Default for PluginSandboxConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: default_max_memory_mb(),
            max_cpu_percent: default_max_cpu_percent(),
            max_connections: default_max_connections(),
            enabled: true,
            max_restarts: default_max_restarts(),
        }
    }
}

/// Current plugin API version. Plugins declaring a higher `minApiVersion` will be rejected.
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
    /// Minimum host API version required by this plugin.
    /// If this field is absent it defaults to 1 (compatible with all hosts).
    #[serde(default = "default_min_api_version")]
    pub min_api_version: u32,
    /// Optional URL where updates can be fetched.
    #[serde(default)]
    pub update_url: String,
    /// Permissions this plugin requires. Checked at runtime.
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    /// Sandbox configuration for process isolation and resource limits.
    #[serde(default)]
    pub sandbox: PluginSandboxConfig,
    /// Path to the plugin executable (relative to plugin directory).
    /// Only required for sandboxed plugins that run as subprocesses.
    #[serde(default)]
    pub entry_point: String,
}

fn default_min_api_version() -> u32 {
    1
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

/// Runtime status of a sandboxed plugin process.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSandboxStatus {
    pub plugin_name: String,
    pub pid: Option<u32>,
    pub state: PluginRunState,
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f64,
    pub restart_count: u32,
    pub uptime_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginRunState {
    Stopped,
    Running,
    Crashed,
    Restarting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginErrorLog {
    pub plugin_name: String,
    pub timestamp: String,
    pub error: String,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn plugins_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    path.push("plugins");
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!("Failed to create plugins directory {:?}: {}", path, e);
    }
    path
}

fn error_log_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    path.push("plugin_errors.json");
    path
}

/// Read the error log from disk.
fn read_error_log() -> Vec<PluginErrorLog> {
    let path = error_log_path();
    if !path.exists() {
        return Vec::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Write the error log to disk, keeping only the last 200 entries.
fn write_error_log(log: &[PluginErrorLog]) {
    let path = error_log_path();
    let trimmed: Vec<_> = if log.len() > 200 {
        log[log.len() - 200..].to_vec()
    } else {
        log.to_vec()
    };
    if let Ok(json) = serde_json::to_string_pretty(&trimmed) {
        let _ = std::fs::write(&path, json);
    }
}

/// Validate that a manifest is compatible with this host.
fn validate_compatibility(manifest: &PluginManifest) -> Result<(), String> {
    if manifest.min_api_version > PLUGIN_API_VERSION {
        return Err(format!(
            "Plugin '{}' requires API version {} but host only supports {}",
            manifest.name, manifest.min_api_version, PLUGIN_API_VERSION
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Record an error for a plugin.
pub fn log_plugin_error(plugin_name: &str, error: &str) {
    let mut log = read_error_log();
    log.push(PluginErrorLog {
        plugin_name: plugin_name.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        error: error.to_string(),
    });
    write_error_log(&log);
}

/// Return the error log for a specific plugin (or all plugins if name is empty).
pub fn get_plugin_errors(plugin_name: &str) -> Vec<PluginErrorLog> {
    let log = read_error_log();
    if plugin_name.is_empty() {
        log
    } else {
        log.into_iter()
            .filter(|e| e.plugin_name == plugin_name)
            .collect()
    }
}

/// Scan plugins directory and return all discovered plugins (only compatible ones).
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
                            if validate_compatibility(&manifest).is_ok() {
                                plugins.push(PluginInfo {
                                    manifest,
                                    path: path.to_string_lossy().to_string(),
                                });
                            } else {
                                tracing::warn!("Skipping incompatible plugin at {:?}", path);
                            }
                        }
                    }
                }
            }
        }
    }

    plugins
}

/// Save plugin enabled/disabled state.
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
                                std::fs::write(&manifest_path, json).map_err(|e| e.to_string())?;
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

/// Install a plugin from a local directory path or a zip file path.
///
/// - If `source` is a directory containing `manifest.json`, it is copied into the plugins dir.
/// - If `source` is a `.zip` file, it is extracted into the plugins dir.
/// - If `source` is a URL (http/https), the zip is downloaded then extracted.
pub async fn install_plugin(source: &str) -> Result<PluginInfo, String> {
    let plugins_root = plugins_dir();

    // Determine if source is a URL
    let is_url = source.starts_with("http://") || source.starts_with("https://");

    if is_url {
        install_from_url(source, &plugins_root).await
    } else {
        let src_path = std::path::Path::new(source);
        if !src_path.exists() {
            return Err(format!("Source path does not exist: {}", source));
        }

        if src_path.is_dir() {
            install_from_dir(src_path, &plugins_root)
        } else if src_path.extension().map_or(false, |e| e == "zip") {
            install_from_zip(src_path, &plugins_root)
        } else {
            Err("Source must be a directory containing manifest.json or a .zip file".to_string())
        }
    }
}

fn install_from_dir(
    src: &std::path::Path,
    plugins_root: &std::path::Path,
) -> Result<PluginInfo, String> {
    let manifest_path = src.join("manifest.json");
    if !manifest_path.exists() {
        return Err("Source directory does not contain manifest.json".to_string());
    }

    let data = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    let manifest: PluginManifest =
        serde_json::from_str(&data).map_err(|e| format!("Invalid manifest.json: {}", e))?;

    validate_compatibility(&manifest)?;

    let dest = plugins_root.join(sanitize_dir_name(&manifest.name));
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }

    copy_dir_recursive(src, &dest).map_err(|e| format!("Failed to copy plugin: {}", e))?;

    Ok(PluginInfo {
        manifest,
        path: dest.to_string_lossy().to_string(),
    })
}

fn install_from_zip(
    zip_path: &std::path::Path,
    plugins_root: &std::path::Path,
) -> Result<PluginInfo, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip: {}", e))?;

    // Find manifest.json inside the zip to determine plugin name
    let manifest_idx = (0..archive.len()).find(|&i| {
        archive
            .by_index(i)
            .ok()
            .map_or(false, |f| f.name().ends_with("manifest.json"))
    });

    let manifest_idx = manifest_idx.ok_or("Zip does not contain manifest.json")?;

    let manifest_content = {
        let mut file = archive.by_index(manifest_idx).map_err(|e| e.to_string())?;
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut file, &mut buf).map_err(|e| e.to_string())?;
        buf
    };

    let manifest: PluginManifest = serde_json::from_str(&manifest_content)
        .map_err(|e| format!("Invalid manifest.json in zip: {}", e))?;

    validate_compatibility(&manifest)?;

    let dest = plugins_root.join(sanitize_dir_name(&manifest.name));
    let manifest_name = archive
        .by_index(manifest_idx)
        .map_err(|e| e.to_string())?
        .name()
        .to_string();
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let outpath = match zip_member_install_path(&dest, file.name(), &manifest_name) {
            Some(p) => p,
            None => continue,
        };

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut outfile = std::fs::File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
        }
    }

    // Re-read manifest from extracted location (in case zip had a prefix dir)
    let final_manifest_path = dest.join("manifest.json");
    let manifest = if final_manifest_path.exists() {
        let data = std::fs::read_to_string(&final_manifest_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).unwrap_or(manifest)
    } else {
        manifest
    };

    Ok(PluginInfo {
        manifest,
        path: dest.to_string_lossy().to_string(),
    })
}

async fn install_from_url(url: &str, plugins_root: &std::path::Path) -> Result<PluginInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download returned status: {}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read download: {}", e))?;

    // Write to temp file then extract
    let tmp_dir = std::env::temp_dir();
    let tmp_file = tmp_dir.join(format!("moon_plugin_{}.zip", uuid::Uuid::new_v4()));
    std::fs::write(&tmp_file, &bytes).map_err(|e| e.to_string())?;

    let result = install_from_zip(&tmp_file, plugins_root);
    let _ = std::fs::remove_file(&tmp_file);
    result
}

/// Uninstall a plugin by name (deletes its directory).
pub fn uninstall_plugin(plugin_name: &str) -> Result<(), String> {
    let dir = plugins_dir();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let manifest_path = path.join("manifest.json");
            if manifest_path.exists() {
                if let Ok(data) = std::fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&data) {
                        if manifest.name == plugin_name {
                            std::fs::remove_dir_all(&path)
                                .map_err(|e| format!("Failed to remove plugin directory: {}", e))?;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Err(format!("Plugin '{}' not found", plugin_name))
}

/// Check for plugin update by fetching the update_url and comparing versions.
/// Returns (has_update, latest_version) if an update is available.
pub async fn check_plugin_update(plugin_name: &str) -> Result<(bool, String), String> {
    let plugins = scan_plugins();
    let plugin = plugins
        .iter()
        .find(|p| p.manifest.name == plugin_name)
        .ok_or_else(|| format!("Plugin '{}' not found", plugin_name))?;

    if plugin.manifest.update_url.is_empty() {
        return Ok((false, plugin.manifest.version.clone()));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&plugin.manifest.update_url)
        .send()
        .await
        .map_err(|e| format!("Update check failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Update check returned status: {}", resp.status()));
    }

    // The update endpoint should return a manifest JSON with at least name and version
    let remote: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse update response: {}", e))?;

    let remote_version = remote
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0");

    let has_update = version_gt(remote_version, &plugin.manifest.version);
    Ok((has_update, remote_version.to_string()))
}

/// Compare two semver-like version strings. Returns true if a > b.
fn version_gt(a: &str, b: &str) -> bool {
    let parse =
        |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse::<u32>().ok()).collect() };
    let va = parse(a);
    let vb = parse(b);
    for i in 0..va.len().max(vb.len()) {
        let a_part = va.get(i).copied().unwrap_or(0);
        let b_part = vb.get(i).copied().unwrap_or(0);
        if a_part > b_part {
            return true;
        }
        if a_part < b_part {
            return false;
        }
    }
    false
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

// ---------------------------------------------------------------------------
// Utility: recursive directory copy
// ---------------------------------------------------------------------------

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

fn sanitize_dir_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Marketplace registry
// ---------------------------------------------------------------------------

/// A single entry in the marketplace registry JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub full_description: String,
    pub author: String,
    pub category: String,
    pub icon: String,
    pub rating: f64,
    pub downloads: u64,
    pub version: String,
    pub latest_version: String,
    pub permissions: Vec<String>,
    pub changelog: Vec<MarketplaceChangelogEntry>,
    /// Optional download URL for the plugin zip.
    #[serde(default)]
    pub download_url: String,
    /// Whether this entry has been installed locally.
    #[serde(default)]
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceChangelogEntry {
    pub version: String,
    pub date: String,
    pub changes: String,
}

fn marketplace_registry_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    path.push("marketplace_registry.json");
    path
}

/// Default marketplace data seeded when no registry file exists.
fn default_marketplace_entries() -> Vec<MarketplaceEntry> {
    vec![
        MarketplaceEntry {
            id: "deep-translator".into(),
            name: "Deep Translator Pro".into(),
            description: "Advanced translation engine with context-aware neural translation support.".into(),
            full_description: "Deep Translator Pro provides high-quality neural machine translation with context awareness. It supports 50+ languages and includes features like domain-specific translation, glossary management, and batch processing. Uses state-of-the-art transformer models for superior fluency.".into(),
            author: "MoonTeam".into(),
            category: "translation".into(),
            icon: "\u{1F310}".into(),
            rating: 4.8,
            downloads: 12500,
            version: "2.1.0".into(),
            latest_version: "2.3.1".into(),
            permissions: vec!["Network access".into(), "Clipboard read/write".into()],
            changelog: vec![
                MarketplaceChangelogEntry { version: "2.3.1".into(), date: "2026-05-20".into(), changes: "Fixed rare crash on long texts".into() },
                MarketplaceChangelogEntry { version: "2.3.0".into(), date: "2026-05-10".into(), changes: "Added Korean language support".into() },
                MarketplaceChangelogEntry { version: "2.2.0".into(), date: "2026-04-15".into(), changes: "Improved context window size".into() },
            ],
            download_url: String::new(),
            installed: false,
        },
        MarketplaceEntry {
            id: "grammar-check".into(),
            name: "Grammar Guard".into(),
            description: "Real-time grammar and style checking for translated text.".into(),
            full_description: "Grammar Guard automatically checks translated text for grammatical errors, awkward phrasing, and style inconsistencies. Supports multiple target languages and integrates seamlessly with the translation pipeline.".into(),
            author: "LinguaTools".into(),
            category: "text-processing".into(),
            icon: "\u{2705}".into(),
            rating: 4.5,
            downloads: 8300,
            version: "1.4.0".into(),
            latest_version: "1.4.0".into(),
            permissions: vec!["Text processing".into()],
            changelog: vec![
                MarketplaceChangelogEntry { version: "1.4.0".into(), date: "2026-04-28".into(), changes: "Added French grammar rules".into() },
            ],
            download_url: String::new(),
            installed: false,
        },
        MarketplaceEntry {
            id: "term-extractor".into(),
            name: "Term Extractor".into(),
            description: "Automatically extract and manage terminology from source and translated texts.".into(),
            full_description: "Term Extractor identifies domain-specific terminology in your texts and helps build consistent glossaries. It uses statistical and linguistic methods to detect key terms, and can auto-populate your glossary for future translations.".into(),
            author: "NLP Labs".into(),
            category: "text-processing".into(),
            icon: "\u{1F4D6}".into(),
            rating: 4.2,
            downloads: 5600,
            version: "1.0.0".into(),
            latest_version: "1.2.0".into(),
            permissions: vec!["Text analysis".into(), "Glossary access".into()],
            changelog: vec![
                MarketplaceChangelogEntry { version: "1.2.0".into(), date: "2026-05-15".into(), changes: "Added multi-language term detection".into() },
                MarketplaceChangelogEntry { version: "1.1.0".into(), date: "2026-04-20".into(), changes: "Improved accuracy for technical texts".into() },
            ],
            download_url: String::new(),
            installed: false,
        },
        MarketplaceEntry {
            id: "theme-pack".into(),
            name: "Theme Pack: Solarized".into(),
            description: "Beautiful Solarized dark and light themes for the translator UI.".into(),
            full_description: "A collection of carefully crafted Solarized color themes. Includes both Solarized Dark and Solarized Light variants, optimized for long reading sessions with reduced eye strain.".into(),
            author: "DesignStudio".into(),
            category: "ui-enhancement".into(),
            icon: "\u{1F3A8}".into(),
            rating: 4.9,
            downloads: 15200,
            version: "3.0.0".into(),
            latest_version: "3.0.0".into(),
            permissions: vec!["UI customization".into()],
            changelog: vec![
                MarketplaceChangelogEntry { version: "3.0.0".into(), date: "2026-05-01".into(), changes: "Complete redesign with new accent colors".into() },
            ],
            download_url: String::new(),
            installed: false,
        },
        MarketplaceEntry {
            id: "ocr-enhanced".into(),
            name: "OCR Enhanced".into(),
            description: "Enhanced OCR engine with better accuracy for CJK characters and handwriting.".into(),
            full_description: "OCR Enhanced replaces the default OCR engine with a more accurate model specifically trained on CJK (Chinese, Japanese, Korean) characters. Also includes handwriting recognition support for stylus-written text.".into(),
            author: "VisionAI".into(),
            category: "translation".into(),
            icon: "\u{1F4F7}".into(),
            rating: 4.6,
            downloads: 9800,
            version: "1.8.0".into(),
            latest_version: "2.0.0".into(),
            permissions: vec!["Screen capture".into(), "OCR processing".into()],
            changelog: vec![
                MarketplaceChangelogEntry { version: "2.0.0".into(), date: "2026-05-22".into(), changes: "Major model upgrade, 30% faster".into() },
                MarketplaceChangelogEntry { version: "1.9.0".into(), date: "2026-05-05".into(), changes: "Added handwriting recognition".into() },
            ],
            download_url: String::new(),
            installed: false,
        },
        MarketplaceEntry {
            id: "shortcut-manager".into(),
            name: "Shortcut Manager".into(),
            description: "Custom keyboard shortcuts and macro support for power users.".into(),
            full_description: "Shortcut Manager allows you to create custom keyboard shortcuts and macros. Chain multiple actions together, create context-aware shortcuts, and export/import shortcut profiles.".into(),
            author: "PowerUser".into(),
            category: "ui-enhancement".into(),
            icon: "\u{2328}\u{FE0F}".into(),
            rating: 4.3,
            downloads: 4100,
            version: "1.1.0".into(),
            latest_version: "1.1.0".into(),
            permissions: vec!["Keyboard hooks".into()],
            changelog: vec![
                MarketplaceChangelogEntry { version: "1.1.0".into(), date: "2026-04-10".into(), changes: "Added macro recording feature".into() },
            ],
            download_url: String::new(),
            installed: false,
        },
    ]
}

/// Load marketplace entries from the local registry JSON file.
/// If the file does not exist, creates it with default data.
pub fn load_marketplace_entries() -> Vec<MarketplaceEntry> {
    let path = marketplace_registry_path();
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(entries) = serde_json::from_str::<Vec<MarketplaceEntry>>(&data) {
                return entries;
            }
        }
    }
    // First run: seed with defaults and persist
    let entries = default_marketplace_entries();
    save_marketplace_entries(&entries);
    entries
}

/// Save marketplace entries back to the registry file.
pub fn save_marketplace_entries(entries: &[MarketplaceEntry]) {
    let path = marketplace_registry_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(entries) {
        let _ = std::fs::write(&path, json);
    }
}

/// Cross-reference marketplace registry with installed plugins to produce the
/// list the frontend needs. Status is computed:
/// - If the plugin directory exists AND version == latestVersion => "installed"
/// - If the plugin directory exists AND version < latestVersion => "update-available"
/// - Otherwise => "available"
pub fn list_marketplace_plugins() -> Vec<MarketplaceEntry> {
    let installed = scan_plugins();
    let installed_map: std::collections::HashMap<String, &PluginInfo> = installed
        .iter()
        .map(|p| (p.manifest.name.clone(), p))
        .collect();

    let mut entries = load_marketplace_entries();
    for entry in &mut entries {
        if let Some(info) = installed_map.get(&entry.name) {
            entry.installed = true;
            entry.version = info.manifest.version.clone();
        } else {
            entry.installed = false;
        }
    }
    entries
}

/// "Install" a marketplace plugin by id. Since there is no real marketplace
/// server, this simply marks the entry as installed in the registry and creates
/// a minimal plugin directory with a manifest so it appears in `scan_plugins`.
pub fn install_marketplace_plugin(id: &str) -> Result<MarketplaceEntry, String> {
    let mut entries = load_marketplace_entries();
    let entry = entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| format!("Marketplace plugin '{}' not found", id))?;

    // Create a minimal plugin directory so scan_plugins picks it up
    let plugins_root = plugins_dir();
    let dest = plugins_root.join(sanitize_dir_name(&entry.name));
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    let manifest = serde_json::json!({
        "name": entry.name,
        "version": entry.latest_version,
        "description": entry.description,
        "author": entry.author,
        "type": "translation",
        "enabled": true,
        "minApiVersion": 1,
        "updateUrl": entry.download_url,
        "permissions": [],
        "sandbox": { "enabled": false },
        "entryPoint": ""
    });
    let manifest_path = dest.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    entry.installed = true;
    entry.version = entry.latest_version.clone();
    let result = entry.clone();
    save_marketplace_entries(&entries);

    Ok(result)
}

/// "Uninstall" a marketplace plugin by id.
pub fn uninstall_marketplace_plugin(id: &str) -> Result<(), String> {
    let mut entries = load_marketplace_entries();
    let entry = entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| format!("Marketplace plugin '{}' not found", id))?;

    // Remove the plugin directory if it exists
    let _ = uninstall_plugin(&entry.name);

    entry.installed = false;
    save_marketplace_entries(&entries);
    Ok(())
}

/// "Update" a marketplace plugin by id. Bumps the local version to latest.
pub fn update_marketplace_plugin(id: &str) -> Result<MarketplaceEntry, String> {
    let mut entries = load_marketplace_entries();
    let entry = entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| format!("Marketplace plugin '{}' not found", id))?;

    // Update manifest version in the plugin directory
    let plugins_root = plugins_dir();
    let dest = plugins_root.join(sanitize_dir_name(&entry.name));
    let manifest_path = dest.join("manifest.json");
    if manifest_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&manifest_path) {
            if let Ok(mut manifest) = serde_json::from_str::<serde_json::Value>(&data) {
                manifest["version"] = serde_json::Value::String(entry.latest_version.clone());
                if let Ok(json) = serde_json::to_string_pretty(&manifest) {
                    let _ = std::fs::write(&manifest_path, json);
                }
            }
        }
    }

    entry.installed = true;
    entry.version = entry.latest_version.clone();
    let result = entry.clone();
    save_marketplace_entries(&entries);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_gt() {
        assert!(version_gt("1.1.0", "1.0.0"));
        assert!(version_gt("2.0.0", "1.9.9"));
        assert!(!version_gt("1.0.0", "1.0.0"));
        assert!(!version_gt("1.0.0", "1.0.1"));
        assert!(version_gt("1.0.10", "1.0.9"));
    }

    #[test]
    fn test_sanitize_dir_name() {
        assert_eq!(sanitize_dir_name("My Plugin!"), "My_Plugin_");
        assert_eq!(sanitize_dir_name("valid-name_123"), "valid-name_123");
    }

    #[test]
    fn test_manifest_install_path_flattens_zip_prefix() {
        let dest = PathBuf::from("plugins").join("Example");

        assert_eq!(
            manifest_install_path(&dest, "example/manifest.json"),
            dest.join("manifest.json")
        );
        assert_eq!(
            manifest_install_path(&dest, "manifest.json"),
            dest.join("manifest.json")
        );
    }

    #[test]
    fn test_zip_member_install_path_strips_manifest_prefix() {
        let dest = PathBuf::from("plugins").join("Example");

        assert_eq!(
            zip_member_install_path(&dest, "example/bin/plugin.exe", "example/manifest.json"),
            Some(dest.join("bin").join("plugin.exe"))
        );
        assert!(zip_member_install_path(&dest, "../evil.exe", "example/manifest.json").is_none());
    }
}

#[cfg(test)]
fn manifest_install_path(dest: &std::path::Path, manifest_name: &str) -> PathBuf {
    let manifest_path = std::path::Path::new(manifest_name);
    let prefix = manifest_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""));
    dest.join(manifest_path.strip_prefix(prefix).unwrap_or(manifest_path))
}

fn zip_member_install_path(
    dest: &std::path::Path,
    member_name: &str,
    manifest_name: &str,
) -> Option<PathBuf> {
    let member_path = std::path::Path::new(member_name);
    let manifest_path = std::path::Path::new(manifest_name);
    let prefix = manifest_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""));
    let relative = member_path.strip_prefix(prefix).unwrap_or(member_path);

    if relative.as_os_str().is_empty() {
        return None;
    }

    let mut safe_relative = PathBuf::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => safe_relative.push(part),
            std::path::Component::CurDir => {},
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => return None,
        }
    }

    if safe_relative.as_os_str().is_empty() {
        None
    } else {
        Some(dest.join(safe_relative))
    }
}

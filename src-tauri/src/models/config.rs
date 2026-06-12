use serde::{Deserialize, Serialize};

// Re-export RoutingStrategy so it's accessible via models::config::RoutingStrategy
pub use super::translation::RoutingStrategy;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    pub name: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: String,
    #[serde(default)]
    pub api_keys: Vec<String>,
    pub base_url: String,
    pub model: String,
}

impl LlmConfig {
    /// Get all API keys (merges api_key + api_keys, deduplicates, removes empty)
    pub fn all_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        if !self.api_key.is_empty() {
            keys.push(self.api_key.clone());
        }
        for k in &self.api_keys {
            if !k.is_empty() && !keys.contains(k) {
                keys.push(k.clone());
            }
        }
        keys
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnginesConfig {
    pub google: GoogleConfig,
    pub baidu: BaiduConfig,
    pub youdao: YoudaoConfig,
    #[serde(default)]
    pub deepl: DeepLConfig,
    #[serde(default)]
    pub deeplx: DeepLXConfig,
    #[serde(default)]
    pub microsoft: MicrosoftConfig,
    #[serde(default)]
    pub yandex: YandexConfig,
    #[serde(default)]
    pub offline: OfflineConfig,
    #[serde(default)]
    pub caiyun: CaiyunConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduConfig {
    pub enabled: bool,
    pub app_id: String,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoudaoConfig {
    pub enabled: bool,
    #[serde(default)]
    pub use_ai: bool,
    /// Youdao OCR API key (default: YoudaoDict built-in key)
    #[serde(default = "default_youdao_ocr_app_key")]
    pub ocr_app_key: String,
    /// Youdao OCR API secret (default: YoudaoDict built-in secret)
    #[serde(default = "default_youdao_ocr_app_secret")]
    pub ocr_app_secret: String,
}

fn default_youdao_ocr_app_key() -> String {
    "3d9fa94028675971".to_string()
}

fn default_youdao_ocr_app_secret() -> String {
    "5X2CJlMERfGOkOP0PFqokVJkSgDIOD0p".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLConfig {
    pub enabled: bool,
    pub api_key: String,
    pub pro: bool,
}

impl Default for DeepLConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            pro: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLXConfig {
    pub enabled: bool,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub pro: bool,
}

impl Default for DeepLXConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: None,
            pro: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrosoftConfig {
    pub enabled: bool,
}

impl Default for MicrosoftConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YandexConfig {
    pub enabled: bool,
}

impl Default for YandexConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineConfig {
    /// Enable offline translation engine
    pub enabled: bool,
    /// Auto-switch to offline when network is unavailable
    #[serde(default = "default_true")]
    pub auto_switch: bool,
    /// Downloaded language pairs (e.g., ["en-zh", "zh-en"])
    #[serde(default)]
    pub downloaded_models: Vec<String>,
    /// Model storage directory (empty = default app data dir)
    #[serde(default)]
    pub model_dir: String,
}

impl Default for OfflineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_switch: true,
            downloaded_models: Vec::new(),
            model_dir: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaiyunConfig {
    pub enabled: bool,
    pub api_token: String,
}

impl Default for CaiyunConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_token: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyConfig {
    pub ocr_translate: String,
    pub show_window: String,
    pub translate_selection: String,
    #[serde(default = "default_replace_hotkey")]
    pub replace_translate: String,
    #[serde(default = "default_overlay_click_through_hotkey")]
    pub toggle_overlay_click_through: String,
}

fn default_overlay_click_through_hotkey() -> String {
    "Ctrl+Shift+Escape".to_string()
}

fn default_replace_hotkey() -> String {
    "Ctrl+Shift+R".to_string()
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            ocr_translate: "Ctrl+Shift+T".to_string(),
            show_window: "Ctrl+T".to_string(),
            translate_selection: "Ctrl+Shift+Y".to_string(),
            replace_translate: "Ctrl+Shift+R".to_string(),
            toggle_overlay_click_through: "Ctrl+Shift+Escape".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    pub enabled: bool,
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_type: "http".to_string(),
            host: String::new(),
            port: 7890,
            username: String::new(),
            password: String::new(),
        }
    }
}

fn default_auto_copy_mode() -> String {
    "translated".to_string()
}

fn default_follow_mode() -> String {
    "none".to_string()
}

fn default_overlay_level() -> u8 {
    2
}

fn default_overlay_auto_dismiss_ms() -> u64 {
    3000
}

fn default_overlay_follow_mode() -> String {
    "none".to_string()
}

fn default_api_port() -> u16 {
    60828
}

fn default_ocr_interval() -> u32 {
    2000
}

fn default_hook_enabled_sources() -> Vec<String> {
    vec![
        "uia".into(),
        "clipboard".into(),
        "ocr".into(),
        "hook".into(),
    ]
}

fn default_uia_interval_ms() -> u64 {
    500
}

fn default_ocr_hook_interval_ms() -> u64 {
    5000
}

fn default_tm_threshold() -> f64 {
    0.8
}

fn default_http_timeout_secs() -> u64 {
    30
}

fn default_ocr_timeout_secs() -> u64 {
    30
}

fn default_ocr_engine() -> String {
    "winrt".to_string() // Changed from "auto" - WinRT is fast and reliable on Windows
}

fn default_llm_timeout_secs() -> u64 {
    120
}

fn default_translation_timeout_secs() -> u64 {
    30
}

fn default_edge_tts_token() -> String {
    "6A5AA1D4EAFF4E9FB37E23D68491D6F4".to_string()
}

fn default_sync_interval_mins() -> u64 {
    30
}

fn default_sync_remote_dir() -> String {
    "moontranslator".to_string()
}

/// WebDAV cloud sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    /// Whether cloud sync is enabled
    #[serde(default)]
    pub enabled: bool,
    /// WebDAV server URL (e.g., "https://dav.jianguoyun.com/dav/")
    #[serde(default)]
    pub server_url: String,
    /// WebDAV username
    #[serde(default)]
    pub username: String,
    /// WebDAV password (encrypted when saved to disk)
    #[serde(default)]
    pub password: String,
    /// Remote directory path on the WebDAV server
    #[serde(default = "default_sync_remote_dir")]
    pub remote_dir: String,
    /// Auto-sync interval in minutes (0 = manual only)
    #[serde(default = "default_sync_interval_mins")]
    pub interval_mins: u64,
    /// Whether to sync config
    #[serde(default = "default_true")]
    pub sync_config: bool,
    /// Whether to sync glossary
    #[serde(default = "default_true")]
    pub sync_glossary: bool,
    /// Whether to sync translation memory (history)
    #[serde(default = "default_true")]
    pub sync_history: bool,
    /// Whether to sync wordbook
    #[serde(default = "default_true")]
    pub sync_wordbook: bool,
    /// Last successful sync timestamp (epoch millis)
    #[serde(default)]
    pub last_sync_at: i64,
    /// Last sync status message
    #[serde(default)]
    pub last_sync_status: String,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: String::new(),
            username: String::new(),
            password: String::new(),
            remote_dir: default_sync_remote_dir(),
            interval_mins: default_sync_interval_mins(),
            sync_config: true,
            sync_glossary: true,
            sync_history: true,
            sync_wordbook: true,
            last_sync_at: 0,
            last_sync_status: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookConfig {
    #[serde(default = "default_hook_enabled_sources")]
    pub enabled_sources: Vec<String>,
    #[serde(default = "default_true")]
    pub show_overlay: bool,
    #[serde(default)]
    pub auto_copy: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// UIA polling interval in milliseconds (default: 500ms)
    #[serde(default = "default_uia_interval_ms")]
    pub uia_interval_ms: u64,
    /// OCR polling interval in milliseconds (default: 5000ms)
    #[serde(default = "default_ocr_hook_interval_ms")]
    pub ocr_interval_ms: u64,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            enabled_sources: default_hook_enabled_sources(),
            show_overlay: true,
            auto_copy: false,
            enabled: true,
            uia_interval_ms: 500,
            ocr_interval_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub llm: LlmConfig,
    pub engines: EnginesConfig,
    pub default_from: String,
    pub default_to: String,
    #[serde(default)]
    pub custom_prompt: String,
    #[serde(default)]
    pub prompt_templates: Vec<PromptTemplate>,
    #[serde(default)]
    pub clipboard_monitor: bool,
    #[serde(default)]
    pub auto_copy_result: bool,
    #[serde(default = "default_auto_copy_mode")]
    pub auto_copy_mode: String,
    #[serde(default)]
    pub translation_mask: bool,
    #[serde(default)]
    pub api_server_enabled: bool,
    #[serde(default = "default_api_port")]
    pub api_server_port: u16,
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub window_x: Option<f64>,
    #[serde(default)]
    pub window_y: Option<f64>,
    #[serde(default)]
    pub window_width: Option<f64>,
    #[serde(default)]
    pub window_height: Option<f64>,
    #[serde(default = "default_follow_mode")]
    pub window_follow_mode: String,
    #[serde(default)]
    pub translation_blacklist: Vec<String>,
    #[serde(default)]
    pub routing_strategy: Option<RoutingStrategy>,
    /// OCR engine preference: "auto", "winrt", "youdao", "tesseract"
    #[serde(default = "default_ocr_engine")]
    pub ocr_engine: String,
    #[serde(default = "default_overlay_level")]
    pub overlay_level: u8,
    #[serde(default = "default_overlay_auto_dismiss_ms")]
    pub overlay_auto_dismiss_ms: u64,
    /// Overlay follow mode: "none", "cursor", "target_bounds"
    /// Separate from window_follow_mode which controls main window behavior.
    #[serde(default = "default_overlay_follow_mode")]
    pub overlay_follow_mode: String,
    /// OCR monitor interval in milliseconds
    #[serde(default = "default_ocr_interval")]
    pub ocr_interval: u32,
    /// OCR overlay click-through by default
    #[serde(default)]
    pub ocr_click_through: bool,
    /// Auto-bind to foreground window when starting OCR monitor
    #[serde(default = "default_true")]
    pub ocr_auto_bind_window: bool,
    /// Hook monitor configuration
    #[serde(default)]
    pub hook: HookConfig,
    /// Translation Memory: enable fuzzy matching from history
    #[serde(default)]
    pub tm_enabled: bool,
    /// Translation Memory: similarity threshold (0.0 - 1.0, default 0.8)
    #[serde(default = "default_tm_threshold")]
    pub tm_threshold: f64,
    /// Furigana: add ruby annotations for Japanese kanji
    #[serde(default)]
    pub furigana_enabled: bool,
    /// TTS: auto-play translation result
    #[serde(default)]
    pub tts_auto_play: bool,
    /// TTS: preferred voice name (empty = auto from language)
    #[serde(default)]
    pub tts_voice: String,
    /// HTTP request timeout in seconds (default: 30)
    #[serde(default = "default_http_timeout_secs")]
    pub http_timeout_secs: u64,
    /// OCR request timeout in seconds (default: 30)
    #[serde(default = "default_ocr_timeout_secs")]
    pub ocr_timeout_secs: u64,
    /// LLM request timeout in seconds (default: 120)
    #[serde(default = "default_llm_timeout_secs")]
    pub llm_timeout_secs: u64,
    /// Translation engine request timeout in seconds (default: 30)
    #[serde(default = "default_translation_timeout_secs")]
    pub translation_timeout_secs: u64,
    /// Edge TTS token (default: built-in token, can be overridden)
    #[serde(default = "default_edge_tts_token")]
    pub edge_tts_token: String,
    /// Cloud sync configuration (WebDAV)
    #[serde(default)]
    pub sync: SyncConfig,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default_values() {
        let config = AppConfig::default();
        assert_eq!(config.llm.provider, "deepseek");
        assert_eq!(config.llm.api_key, "");
        assert!(config.llm.api_keys.is_empty());
        assert_eq!(config.llm.base_url, "https://api.deepseek.com/v1");
        assert_eq!(config.llm.model, "deepseek-chat");
        assert_eq!(config.default_from, "auto");
        assert_eq!(config.default_to, "zh");
        assert!(config.custom_prompt.is_empty());
        assert!(!config.clipboard_monitor);
        assert!(!config.auto_copy_result);
        assert_eq!(config.auto_copy_mode, "translated");
        assert!(!config.translation_mask);
        assert!(!config.api_server_enabled);
        assert_eq!(config.api_server_port, 60828);
        assert!(config.translation_blacklist.is_empty());
        assert!(config.routing_strategy.is_none());
        assert_eq!(config.overlay_level, 2);
        assert_eq!(config.overlay_auto_dismiss_ms, 3000);
        assert_eq!(config.overlay_follow_mode, "none");
        assert_eq!(config.ocr_interval, 2000);
        assert!(!config.ocr_click_through);
        assert!(config.ocr_auto_bind_window);
        assert!(!config.tm_enabled);
        assert_eq!(config.tm_threshold, 0.8);
        assert!(!config.furigana_enabled);
        assert!(!config.tts_auto_play);
        assert_eq!(config.http_timeout_secs, 30);
        assert_eq!(config.ocr_timeout_secs, 30);
        assert_eq!(config.llm_timeout_secs, 120);
        assert_eq!(config.translation_timeout_secs, 30);
    }

    #[test]
    fn test_app_config_engines_defaults() {
        let config = AppConfig::default();
        assert!(config.engines.google.enabled);
        assert!(!config.engines.baidu.enabled);
        assert!(config.engines.baidu.app_id.is_empty());
        assert!(config.engines.baidu.secret.is_empty());
        assert!(config.engines.youdao.enabled);
        assert!(!config.engines.youdao.use_ai);
        assert_eq!(config.engines.youdao.ocr_app_key, "3d9fa94028675971");
        assert_eq!(
            config.engines.youdao.ocr_app_secret,
            "5X2CJlMERfGOkOP0PFqokVJkSgDIOD0p"
        );
        assert!(!config.engines.deepl.enabled);
        assert!(!config.engines.deeplx.enabled);
        assert!(!config.engines.microsoft.enabled);
        assert!(!config.engines.yandex.enabled);
        assert!(!config.engines.offline.enabled);
        assert!(config.engines.offline.auto_switch);
    }

    #[test]
    fn test_hotkey_config_defaults() {
        let config = HotkeyConfig::default();
        assert_eq!(config.ocr_translate, "Ctrl+Shift+T");
        assert_eq!(config.show_window, "Ctrl+T");
        assert_eq!(config.translate_selection, "Ctrl+Shift+Y");
        assert_eq!(config.replace_translate, "Ctrl+Shift+R");
        assert_eq!(config.toggle_overlay_click_through, "Ctrl+Shift+Escape");
    }

    #[test]
    fn test_proxy_config_defaults() {
        let config = ProxyConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.proxy_type, "http");
        assert!(config.host.is_empty());
        assert_eq!(config.port, 7890);
        assert!(config.username.is_empty());
        assert!(config.password.is_empty());
    }

    #[test]
    fn test_sync_config_defaults() {
        let config = SyncConfig::default();
        assert!(!config.enabled);
        assert!(config.server_url.is_empty());
        assert!(config.username.is_empty());
        assert!(config.password.is_empty());
        assert_eq!(config.remote_dir, "moontranslator");
        assert_eq!(config.interval_mins, 30);
        assert!(config.sync_config);
        assert!(config.sync_glossary);
        assert!(config.sync_history);
        assert!(config.sync_wordbook);
        assert_eq!(config.last_sync_at, 0);
        assert!(config.last_sync_status.is_empty());
    }

    #[test]
    fn test_hook_config_defaults() {
        let config = HookConfig::default();
        assert!(config.enabled_sources.contains(&"uia".to_string()));
        assert!(config.enabled_sources.contains(&"clipboard".to_string()));
        assert!(config.enabled_sources.contains(&"ocr".to_string()));
        assert!(config.enabled_sources.contains(&"hook".to_string()));
        assert!(config.show_overlay);
        assert!(!config.auto_copy);
        assert!(config.enabled);
        assert_eq!(config.uia_interval_ms, 500);
        assert_eq!(config.ocr_interval_ms, 5000);
    }

    #[test]
    fn test_offline_config_defaults() {
        let config = OfflineConfig::default();
        assert!(!config.enabled);
        assert!(config.auto_switch);
        assert!(config.downloaded_models.is_empty());
        assert!(config.model_dir.is_empty());
    }

    #[test]
    fn test_deepl_config_defaults() {
        let config = DeepLConfig::default();
        assert!(!config.enabled);
        assert!(config.api_key.is_empty());
        assert!(!config.pro);
    }

    #[test]
    fn test_deeplx_config_defaults() {
        let config = DeepLXConfig::default();
        assert!(!config.enabled);
        assert!(config.api_key.is_none());
        assert!(!config.pro);
    }

    #[test]
    fn test_llm_config_all_keys_empty() {
        let llm = LlmConfig {
            provider: "test".to_string(),
            api_key: String::new(),
            api_keys: Vec::new(),
            base_url: String::new(),
            model: String::new(),
        };
        assert!(llm.all_keys().is_empty());
    }

    #[test]
    fn test_llm_config_all_keys_single() {
        let llm = LlmConfig {
            provider: "test".to_string(),
            api_key: "key1".to_string(),
            api_keys: Vec::new(),
            base_url: String::new(),
            model: String::new(),
        };
        assert_eq!(llm.all_keys(), vec!["key1"]);
    }

    #[test]
    fn test_llm_config_all_keys_multiple() {
        let llm = LlmConfig {
            provider: "test".to_string(),
            api_key: "key1".to_string(),
            api_keys: vec!["key2".to_string(), "key3".to_string()],
            base_url: String::new(),
            model: String::new(),
        };
        let keys = llm.all_keys();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0], "key1");
        assert_eq!(keys[1], "key2");
        assert_eq!(keys[2], "key3");
    }

    #[test]
    fn test_llm_config_all_keys_dedup() {
        let llm = LlmConfig {
            provider: "test".to_string(),
            api_key: "key1".to_string(),
            api_keys: vec!["key1".to_string(), "key2".to_string()],
            base_url: String::new(),
            model: String::new(),
        };
        let keys = llm.all_keys();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0], "key1");
        assert_eq!(keys[1], "key2");
    }

    #[test]
    fn test_llm_config_all_keys_skip_empty() {
        let llm = LlmConfig {
            provider: "test".to_string(),
            api_key: String::new(),
            api_keys: vec!["".to_string(), "key2".to_string(), "".to_string()],
            base_url: String::new(),
            model: String::new(),
        };
        let keys = llm.all_keys();
        assert_eq!(keys, vec!["key2"]);
    }

    #[test]
    fn test_routing_strategy_default() {
        assert_eq!(RoutingStrategy::default(), RoutingStrategy::FallbackOnError);
    }

    #[test]
    fn test_app_config_serialize_deserialize_roundtrip() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.llm.provider, config.llm.provider);
        assert_eq!(deserialized.llm.model, config.llm.model);
        assert_eq!(deserialized.default_from, config.default_from);
        assert_eq!(deserialized.default_to, config.default_to);
        assert_eq!(
            deserialized.hotkeys.ocr_translate,
            config.hotkeys.ocr_translate
        );
        assert_eq!(deserialized.proxy.port, config.proxy.port);
        assert_eq!(deserialized.sync.remote_dir, config.sync.remote_dir);
        assert_eq!(deserialized.overlay_level, config.overlay_level);
    }

    #[test]
    fn test_app_config_deserialize_partial_json() {
        // Simulate loading an old config with missing fields
        let json = r#"{
            "llm": {
                "provider": "openai",
                "apiKey": "test-key",
                "baseUrl": "https://api.openai.com/v1",
                "model": "gpt-4"
            },
            "engines": {
                "google": {"enabled": true},
                "baidu": {"enabled": false, "appId": "", "secret": ""},
                "youdao": {"enabled": false, "ocrAppKey": "k", "ocrAppSecret": "s"}
            },
            "defaultFrom": "en",
            "defaultTo": "zh"
        }"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.llm.provider, "openai");
        assert_eq!(config.llm.model, "gpt-4");
        assert_eq!(config.default_from, "en");
        // Missing fields should get defaults
        assert_eq!(config.overlay_level, 2);
        assert_eq!(config.api_server_port, 60828);
        assert!(config.translation_blacklist.is_empty());
        assert_eq!(config.proxy.port, 7890);
        assert_eq!(config.sync.remote_dir, "moontranslator");
        assert!(config.hook.enabled);
    }

    #[test]
    fn test_prompt_template_serde() {
        let template = PromptTemplate {
            name: "test".to_string(),
            prompt: "Translate {{text}}".to_string(),
        };
        let json = serde_json::to_string(&template).unwrap();
        let deserialized: PromptTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.prompt, "Translate {{text}}");
    }

    #[test]
    fn test_app_config_serialize_camel_case() {
        let config = AppConfig::default();
        let json_str = serde_json::to_string(&config).unwrap();
        // Verify camelCase keys are used
        assert!(json_str.contains("defaultFrom"));
        assert!(json_str.contains("defaultTo"));
        assert!(json_str.contains("apiKey"));
        assert!(json_str.contains("baseUrl"));
        assert!(json_str.contains("autoCopyResult"));
        assert!(json_str.contains("translationBlacklist"));
        assert!(json_str.contains("apiServerEnabled"));
        assert!(json_str.contains("apiServerPort"));
        assert!(json_str.contains("overlayLevel"));
        assert!(!json_str.contains("default_from"));
        assert!(!json_str.contains("api_key"));
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig {
                provider: "deepseek".into(),
                api_key: String::new(),
                api_keys: Vec::new(),
                base_url: "https://api.deepseek.com/v1".into(),
                model: "deepseek-chat".into(),
            },
            engines: EnginesConfig {
                google: GoogleConfig { enabled: true },
                baidu: BaiduConfig {
                    enabled: false,
                    app_id: String::new(),
                    secret: String::new(),
                },
                youdao: YoudaoConfig {
                    enabled: true,
                    use_ai: false,
                    ocr_app_key: default_youdao_ocr_app_key(),
                    ocr_app_secret: default_youdao_ocr_app_secret(),
                },
                deepl: DeepLConfig::default(),
                deeplx: DeepLXConfig::default(),
                microsoft: MicrosoftConfig::default(),
                yandex: YandexConfig::default(),
                offline: OfflineConfig::default(),
                caiyun: CaiyunConfig::default(),
            },
            default_from: "auto".into(),
            default_to: "zh".into(),
            custom_prompt: String::new(),
            prompt_templates: Vec::new(),
            clipboard_monitor: false,
            auto_copy_result: false,
            auto_copy_mode: "translated".to_string(),
            translation_mask: false,
            api_server_enabled: false,
            api_server_port: 60828,
            hotkeys: HotkeyConfig::default(),
            proxy: ProxyConfig::default(),
            window_x: None,
            window_y: None,
            window_width: None,
            window_height: None,
            window_follow_mode: "none".to_string(),
            translation_blacklist: Vec::new(),
            routing_strategy: None,
            ocr_engine: default_ocr_engine(),
            overlay_level: 2,
            overlay_auto_dismiss_ms: 3000,
            overlay_follow_mode: "none".to_string(),
            ocr_interval: 2000,
            ocr_click_through: false,
            ocr_auto_bind_window: true,
            hook: HookConfig::default(),
            tm_enabled: false,
            tm_threshold: 0.8,
            furigana_enabled: false,
            tts_auto_play: false,
            tts_voice: String::new(),
            http_timeout_secs: default_http_timeout_secs(),
            ocr_timeout_secs: default_ocr_timeout_secs(),
            llm_timeout_secs: default_llm_timeout_secs(),
            translation_timeout_secs: default_translation_timeout_secs(),
            edge_tts_token: default_edge_tts_token(),
            sync: SyncConfig::default(),
        }
    }
}

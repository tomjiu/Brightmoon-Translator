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

fn default_api_port() -> u16 {
    60828
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
    #[serde(default = "default_overlay_level")]
    pub overlay_level: u8,
    #[serde(default = "default_overlay_auto_dismiss_ms")]
    pub overlay_auto_dismiss_ms: u64,
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
                    enabled: false,
                    use_ai: false,
                },
                deepl: DeepLConfig::default(),
                deeplx: DeepLXConfig::default(),
                microsoft: MicrosoftConfig::default(),
                yandex: YandexConfig::default(),
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
            overlay_level: 2,
            overlay_auto_dismiss_ms: 3000,
        }
    }
}

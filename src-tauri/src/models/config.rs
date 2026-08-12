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
    /// 多提供商配置（用于回退机制）
    #[serde(default)]
    pub providers: Vec<LlmProviderEntry>,
}

/// 单个 LLM 提供商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderEntry {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub models: Vec<String>, // 缓存的可用模型列表
    /// Request wire format: "openai" | "anthropic" | "gemini"
    #[serde(default = "default_api_format")]
    pub api_format: String,
}

/// Resolved LLM endpoint for Router / `LlmEngine` (key + URL + model + format).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmEndpoint {
    pub label: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub api_format: String,
}

fn default_priority() -> i32 {
    0
}

fn default_api_format() -> String {
    "openai".into()
}

pub fn normalize_api_format(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => "anthropic".into(),
        "gemini" | "google" => "gemini".into(),
        _ => "openai".into(),
    }
}

impl LlmConfig {
    /// Get all API keys (merges `api_key` + `api_keys`, deduplicates, removes empty)
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

    /// Enabled providers with non-empty `api_key`, sorted by priority ascending
    /// (lower number = tried first). If none, fall back to top-level `api_key/api_keys` + `base_url` + model.
    pub fn resolve_endpoints(&self) -> Vec<LlmEndpoint> {
        let mut from_providers: Vec<&LlmProviderEntry> = self
            .providers
            .iter()
            .filter(|p| p.enabled && !p.api_key.trim().is_empty())
            .collect();
        from_providers.sort_by_key(|p| p.priority);

        if !from_providers.is_empty() {
            return from_providers
                .into_iter()
                .map(|p| LlmEndpoint {
                    label: if p.name.is_empty() {
                        p.id.clone()
                    } else {
                        p.name.clone()
                    },
                    api_key: p.api_key.trim().to_string(),
                    base_url: if p.base_url.trim().is_empty() {
                        self.base_url.clone()
                    } else {
                        p.base_url.trim().to_string()
                    },
                    model: if p.model.trim().is_empty() {
                        self.model.clone()
                    } else {
                        p.model.trim().to_string()
                    },
                    api_format: normalize_api_format(&p.api_format),
                })
                .collect();
        }

        // Legacy top-level keys (same base_url/model for each key)
        self.all_keys()
            .into_iter()
            .enumerate()
            .map(|(i, api_key)| LlmEndpoint {
                label: if i == 0 {
                    self.provider.clone()
                } else {
                    format!("{}#{}", self.provider, i + 1)
                },
                api_key,
                base_url: self.base_url.clone(),
                model: self.model.clone(),
                api_format: "openai".into(),
            })
            .filter(|e| !e.api_key.is_empty())
            .collect()
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
    /// Tatoeba example sentences (not MT).
    #[serde(default)]
    pub tatoeba: SimpleToggleEngine,
    /// Baidu free web (unofficial).
    #[serde(default, rename = "baiduWeb")]
    pub baidu_web: SimpleToggleEngine,
    /// Caiyun free web JWT path (unofficial).
    #[serde(default, rename = "caiyunWeb")]
    pub caiyun_web: SimpleToggleEngine,
    /// Volcengine CRX free path (unofficial).
    #[serde(default, rename = "volcengineWeb")]
    pub volcengine_web: SimpleToggleEngine,
    /// Tencent `TranSmart` free API.
    #[serde(default)]
    pub transmart: SimpleToggleEngine,
    /// Naver Papago free web.
    #[serde(default)]
    pub papago: SimpleToggleEngine,
}

/// Enabled-only engine config (no credentials).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct SimpleToggleEngine {
    #[serde(default)]
    pub enabled: bool,
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
    /// Youdao OCR API key (default: `YoudaoDict` built-in key)
    #[serde(default = "default_youdao_ocr_app_key")]
    pub ocr_app_key: String,
    /// Youdao OCR API secret (default: `YoudaoDict` built-in secret)
    #[serde(default = "default_youdao_ocr_app_secret")]
    pub ocr_app_secret: String,
}

fn default_youdao_ocr_app_key() -> String {
    String::new()
}

fn default_youdao_ocr_app_secret() -> String {
    String::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct DeepLConfig {
    pub enabled: bool,
    pub api_key: String,
    pub pro: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct DeepLXConfig {
    pub enabled: bool,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub pro: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct MicrosoftConfig {
    pub enabled: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct YandexConfig {
    pub enabled: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineConfig {
    /// Enable offline translation engine
    pub enabled: bool,
    /// Auto-switch to offline when network is unavailable
    #[serde(default = "default_true")]
    pub auto_switch: bool,
    /// Downloaded language pairs (e.g. `en-zh`, `zh-en`)
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
#[derive(Default)]
pub struct CaiyunConfig {
    pub enabled: bool,
    pub api_token: String,
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
    /// Optional QTranslate-style dictionary hotkey (empty = disabled).
    /// Looks up selection as a word; falls through to MT on miss.
    #[serde(default)]
    pub dictionary_lookup: String,
}

/// How selection translation is triggered (Youdao-like UX).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SelectionTriggerMode {
    /// Only global hotkey / tray / HTTP
    HotkeyOnly,
    /// After mouse button release, if there is a selection → translate + overlay
    AutoOnSelect,
    /// After drag-select → floating pop button; click to translate (Easydict default)
    #[default]
    PopButton,
}

/// Desktop 划词 / 取词 UX (Youdao-inspired).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionUxConfig {
    /// Select text → popup: hotkey only vs auto on mouse-up
    #[serde(default)]
    pub trigger_mode: SelectionTriggerMode,
    /// Mouse dwell → word dictionary popup (system-wide)
    #[serde(default)]
    pub hover_dictionary: bool,
    /// Whether hover dictionary also picks up CJK (Chinese) words.
    /// Default off — CJK hover often misfires on UI chrome and triggers slow
    /// LLM fallbacks; selection (划词) is the preferred path for Chinese.
    #[serde(default)]
    pub hover_cjk: bool,
    /// Dwell time before hover dictionary (ms)
    #[serde(default = "default_hover_dwell_ms")]
    pub hover_dwell_ms: u32,
    /// Hover unit: "word" (default) | "sentence" (MTT-style). Alt held can force sentence.
    #[serde(default = "default_hover_unit")]
    pub hover_unit: String,
    /// Hover dictionary backend: auto | ecdict | youdao
    /// auto = ECDICT first (EN) then Youdao; ecdict = local only; youdao = online only
    #[serde(default = "default_hover_dict_source")]
    pub hover_dict_source: String,
    /// When UIA/clipboard selection is empty, try OCR near cursor (force 取词)
    #[serde(default)]
    pub ocr_force_pickup: bool,
    /// Modifier required for OCR force pickup (MTT-style). Empty/`none` = no gate.
    /// Values: "" | "none" | "shift" | "ctrl" | "alt"
    #[serde(default = "default_ocr_modifier_key")]
    pub ocr_modifier_key: String,
    /// Min selection length for auto-on-select (chars)
    #[serde(default = "default_selection_min_chars")]
    pub auto_min_chars: u32,
    /// Min mouse drag distance (px) before auto-on-select fires (Easydict `MinDragDistance`)
    #[serde(default = "default_min_drag_px")]
    pub min_drag_px: u32,
    /// Process names (no .exe) to never auto/hover-select, e.g. "potplayer"
    #[serde(default)]
    pub exclude_processes: Vec<String>,
}

fn default_hover_dwell_ms() -> u32 {
    400
}

fn default_selection_min_chars() -> u32 {
    1
}

fn default_min_drag_px() -> u32 {
    // Easydict MinDragDistance = 10; double-click (0 drag) still accepted if text exists
    10
}

fn default_ocr_modifier_key() -> String {
    // Default none: only ocr_force_pickup switch gates OCR (backward compatible).
    // Recommend "shift" in settings when force pickup is noisy.
    String::new()
}

fn default_hover_unit() -> String {
    "word".into()
}

fn default_hover_dict_source() -> String {
    "auto".into()
}

impl Default for SelectionUxConfig {
    fn default() -> Self {
        Self {
            // Match UI + Easydict: pop button by default
            trigger_mode: SelectionTriggerMode::AutoOnSelect,
            hover_dictionary: false,
            hover_cjk: false,
            hover_dwell_ms: default_hover_dwell_ms(),
            hover_unit: default_hover_unit(),
            hover_dict_source: default_hover_dict_source(),
            ocr_force_pickup: false,
            ocr_modifier_key: default_ocr_modifier_key(),
            auto_min_chars: default_selection_min_chars(),
            min_drag_px: default_min_drag_px(),
            exclude_processes: Vec::new(),
        }
    }
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
            // Empty until user sets (e.g. Ctrl+Shift+D) — optional feature
            dictionary_lookup: String::new(),
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
    // UIA + clipboard only. Hook-internal OCR races the screenshot OCR path;
    // raw winevent hook is noisy — both are opt-in in the UI.
    vec!["uia".into(), "clipboard".into()]
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

/// S5-10: default cache TTL in hours (3 days). Used by `TranslationCache`.
fn default_cache_ttl_hours() -> i64 {
    72
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

fn default_pdf_extraction_engine() -> String {
    "pdf-extract".into()
}

/// Offline OCR sidecar (Rapid / Paddle) — models external.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineOcrConfig {
    /// "rapid" | "paddle"
    #[serde(default = "default_offline_ocr_backend")]
    pub backend: String,
    /// Directory with `RapidOcrOnnx` / PaddleOCR-json + models.
    #[serde(default)]
    pub plugin_dir: String,
}

fn default_offline_ocr_backend() -> String {
    "rapid".into()
}

impl Default for OfflineOcrConfig {
    fn default() -> Self {
        Self {
            backend: default_offline_ocr_backend(),
            plugin_dir: String::new(),
        }
    }
}

/// External PDF text extractors (`MinerU` / Marker / `OCRmyPDF`). Empty = bare command on PATH.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfExtractionSidecarConfig {
    #[serde(default)]
    pub mineru_cmd: String,
    #[serde(default)]
    pub marker_cmd: String,
    #[serde(default)]
    pub ocrmypdf_cmd: String,
}

fn default_llm_timeout_secs() -> u64 {
    120
}

fn default_llm_temperature() -> f32 {
    0.3
}

fn default_llm_max_tokens() -> u32 {
    4096
}

fn default_translation_timeout_secs() -> u64 {
    30
}

fn default_tts_provider() -> String {
    "edge".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiTtsConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_openai_tts_base")]
    pub base_url: String,
    #[serde(default = "default_openai_tts_model")]
    pub model: String,
    #[serde(default = "default_openai_tts_voice")]
    pub voice: String,
    #[serde(default = "default_openai_tts_speed")]
    pub speed: f32,
}

fn default_openai_tts_base() -> String {
    "https://api.openai.com/v1".into()
}
fn default_openai_tts_model() -> String {
    "tts-1".into()
}
fn default_openai_tts_voice() -> String {
    "alloy".into()
}
fn default_openai_tts_speed() -> f32 {
    1.0
}

impl Default for OpenAiTtsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: default_openai_tts_base(),
            model: default_openai_tts_model(),
            voice: default_openai_tts_voice(),
            speed: default_openai_tts_speed(),
        }
    }
}

/// Fish Audio TTS (<https://docs.fish.audio>) — free model `s2.1-pro-free` for dev/test.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FishTtsConfig {
    /// Bearer token; also accepts env `FISH_API_KEY` when empty.
    #[serde(default)]
    pub api_key: String,
    /// Header `model`: s2.1-pro-free | s2.1-pro | s2-pro | s1
    #[serde(default = "default_fish_tts_model")]
    pub model: String,
    /// Voice library / clone model id (`reference_id`)
    #[serde(default = "default_fish_tts_reference_id")]
    pub reference_id: String,
    #[serde(default = "default_fish_tts_format")]
    pub format: String,
    #[serde(default = "default_fish_tts_speed")]
    pub speed: f32,
}

fn default_fish_tts_model() -> String {
    "s2.1-pro-free".into()
}
fn default_fish_tts_reference_id() -> String {
    // Docs sample voice model id for quick try
    "12b8a0bf8e0042c3b11e519d11db8b68".into()
}
fn default_fish_tts_format() -> String {
    "mp3".into()
}
fn default_fish_tts_speed() -> f32 {
    1.0
}

impl Default for FishTtsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_fish_tts_model(),
            reference_id: default_fish_tts_reference_id(),
            format: default_fish_tts_format(),
            speed: default_fish_tts_speed(),
        }
    }
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

/// `WebDAV` cloud sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    /// Whether cloud sync is enabled
    #[serde(default)]
    pub enabled: bool,
    /// `WebDAV` server URL (e.g., "<https://dav.jianguoyun.com/dav>/")
    #[serde(default)]
    pub server_url: String,
    /// `WebDAV` username
    #[serde(default)]
    pub username: String,
    /// `WebDAV` password (encrypted when saved to disk)
    #[serde(default)]
    pub password: String,
    /// Remote directory path on the `WebDAV` server
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
    /// Replace delivery: true = clipboard+Ctrl+V (default); false = Unicode type (STranslate-style).
    #[serde(default = "default_true")]
    pub use_clipboard_output: bool,
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
    /// Shared secret for local HTTP API (`Authorization: Bearer …` or `X-Api-Token`).
    /// Empty while API is off; auto-filled when the server starts if still empty.
    #[serde(default)]
    pub api_server_token: String,
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    /// 划词 / 悬停词典 / OCR 强力取词
    #[serde(default)]
    pub selection_ux: SelectionUxConfig,
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
    /// Engine execution order for fallback routing (e.g. `llm`, `youdao`, `google`)
    #[serde(default)]
    pub engine_order: Vec<String>,
    /// OCR engine preference: "auto", "winrt", "youdao", "tesseract", "rapid", "paddle"
    #[serde(default = "default_ocr_engine")]
    pub ocr_engine: String,
    /// Offline OCR sidecar paths (when `ocr_engine` is rapid/paddle).
    #[serde(default)]
    pub offline_ocr: OfflineOcrConfig,
    /// PDF text extraction: "pdf-extract" | "ocr" | "mineru" | "marker" | "ocrmypdf"
    #[serde(default = "default_pdf_extraction_engine")]
    pub pdf_extraction_engine: String,
    /// Optional CLI paths for mineru/marker/ocrmypdf.
    #[serde(default)]
    pub pdf_extraction_sidecar: PdfExtractionSidecarConfig,
    #[serde(default = "default_overlay_level")]
    pub overlay_level: u8,
    #[serde(default = "default_overlay_auto_dismiss_ms")]
    pub overlay_auto_dismiss_ms: u64,
    /// Overlay follow mode: "none", "cursor", "`target_bounds`"
    /// Separate from `window_follow_mode` which controls main window behavior.
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
    /// S5-10: Translation cache TTL in hours (default: 72 = 3 days).
    /// Entries older than this are evicted on the next cache read.
    #[serde(default = "default_cache_ttl_hours")]
    pub cache_ttl_hours: i64,
    /// Furigana: add ruby annotations for Japanese kanji
    #[serde(default)]
    pub furigana_enabled: bool,
    /// TTS: auto-play translation result
    #[serde(default)]
    pub tts_auto_play: bool,
    /// TTS: preferred voice name (empty = auto from language)
    #[serde(default)]
    pub tts_voice: String,
    /// TTS provider: "edge" | "openai" | "youdao" | "fish"
    #[serde(default = "default_tts_provider")]
    pub tts_provider: String,
    /// Preferred engine id for `BatchManager` when BatchConfig.engine is unset (e.g. "google").
    #[serde(default)]
    pub batch_preferred_engine: String,
    /// OpenAI-compatible TTS settings
    #[serde(default)]
    pub openai_tts: OpenAiTtsConfig,
    /// Fish Audio TTS (s2.1-pro-free etc.)
    #[serde(default)]
    pub fish_tts: FishTtsConfig,
    /// HTTP request timeout in seconds (default: 30)
    #[serde(default = "default_http_timeout_secs")]
    pub http_timeout_secs: u64,
    /// OCR request timeout in seconds (default: 30)
    #[serde(default = "default_ocr_timeout_secs")]
    pub ocr_timeout_secs: u64,
    /// LLM request timeout in seconds (default: 120)
    #[serde(default = "default_llm_timeout_secs")]
    pub llm_timeout_secs: u64,
    /// LLM temperature (creativity) - 0.0 to 2.0, default 0.3
    #[serde(default = "default_llm_temperature")]
    pub llm_temperature: f32,
    /// LLM max tokens (output limit), default 4096
    #[serde(default = "default_llm_max_tokens")]
    pub llm_max_tokens: u32,
    /// Translation engine request timeout in seconds (default: 30)
    #[serde(default = "default_translation_timeout_secs")]
    pub translation_timeout_secs: u64,
    /// Edge TTS token (default: built-in token, can be overridden)
    #[serde(default = "default_edge_tts_token")]
    pub edge_tts_token: String,
    /// Cloud sync configuration (`WebDAV`)
    #[serde(default)]
    pub sync: SyncConfig,
    /// External vocabulary collection (Eudic / Anki / Shanbay / Youdao / Maimemo). Not FSRS learning.
    #[serde(default)]
    pub collection: CollectionConfig,
    /// P6: Enable DocLayout-YOLO layout detection for PDF translation.
    /// Default false — model (~50MB) is downloaded on demand when enabled.
    #[serde(default)]
    pub layout_detection_enabled: bool,
    /// Tier4-6: Run `WinRT` OCR in a one-shot subprocess so the OS reclaims
    /// the ONNX model memory when the child exits. Slower per-call (~200ms
    /// spawn overhead) but bounded memory for occasional-OCR users.
    /// Default false — in-process path is faster for heavy continuous OCR.
    #[serde(default)]
    pub winrt_ocr_use_subprocess: bool,
    /// How many hidden WebView windows to preload at startup (0-3, default 1).
    /// Priority: 1 = translate-card, 2 = +ocr-region-frame, 3 = +selection-pop.
    /// Each preloaded window keeps a renderer process alive (~80-140MB).
    #[serde(default = "default_hot_load_page_count")]
    pub hot_load_page_count: u8,
    /// Defer screenshot-warmup + OCR hot-start until first use instead of
    /// running them ~1s after startup. Startup gets quieter; first OCR call
    /// pays the capture/model-load cost once.
    #[serde(default = "default_defer_startup_warmup")]
    pub defer_startup_warmup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionConfig {
    #[serde(default)]
    pub eudic: EudicCollectionConfig,
    #[serde(default)]
    pub anki: AnkiCollectionConfig,
    #[serde(default)]
    pub shanbay: ShanbayCollectionConfig,
    #[serde(default)]
    pub youdao: YoudaoCollectionConfig,
    #[serde(default)]
    pub maimemo: MaimemoCollectionConfig,
    /// After local wordbook add, also push to enabled remote targets.
    #[serde(default = "default_true")]
    pub auto_push_on_save: bool,
}

impl Default for CollectionConfig {
    fn default() -> Self {
        Self {
            eudic: EudicCollectionConfig::default(),
            anki: AnkiCollectionConfig::default(),
            shanbay: ShanbayCollectionConfig::default(),
            youdao: YoudaoCollectionConfig::default(),
            maimemo: MaimemoCollectionConfig::default(),
            auto_push_on_save: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EudicCollectionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: String,
    #[serde(default = "default_eudic_book")]
    pub book_name: String,
}

fn default_eudic_book() -> String {
    "Moon".into()
}

impl Default for EudicCollectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token: String::new(),
            book_name: default_eudic_book(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnkiCollectionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_anki_port")]
    pub port: u16,
    #[serde(default = "default_anki_deck")]
    pub deck: String,
    #[serde(default = "default_anki_model")]
    pub model: String,
}

fn default_anki_port() -> u16 {
    8765
}
fn default_anki_deck() -> String {
    "Moon".into()
}
fn default_anki_model() -> String {
    "Moon Card".into()
}

impl Default for AnkiCollectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_anki_port(),
            deck: default_anki_deck(),
            model: default_anki_model(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct ShanbayCollectionConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Cookie `auth_token` from Shanbay web login (see pot-app-collection-plugin-shanbay).
    #[serde(default)]
    pub credential: String,
    #[serde(default)]
    pub wordbook_id: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoudaoCollectionConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Full Cookie header from dict.youdao.com after login (pot-app-collection-plugin-youdao).
    #[serde(default)]
    pub cookie: String,
    #[serde(default = "default_youdao_lan")]
    pub lan: String,
}

fn default_youdao_lan() -> String {
    "en".into()
}

impl Default for YoudaoCollectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cookie: String::new(),
            lan: default_youdao_lan(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaimemoCollectionConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Open API token from 墨墨 App 实验功能.
    #[serde(default)]
    pub token: String,
    /// Cloud notepad id; empty → create on first push.
    #[serde(default)]
    pub notepad_id: String,
    #[serde(default = "default_maimemo_title")]
    pub notepad_title: String,
}

fn default_maimemo_title() -> String {
    "Moon".into()
}

impl Default for MaimemoCollectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token: String::new(),
            notepad_id: String::new(),
            notepad_title: default_maimemo_title(),
        }
    }
}

/// Default number of hidden WebView windows preloaded at startup for instant
/// first-use (0-3). Priority: 1 = translate-card, 2 = +ocr-region-frame,
/// 3 = +selection-pop. Mirrors snow-shot's `hotLoadPageCount` pool limit —
/// each preloaded window costs a ~80-140MB renderer process.
fn default_hot_load_page_count() -> u8 {
    1
}

fn default_defer_startup_warmup() -> bool {
    true
}

fn default_true() -> bool {
    true
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
                providers: Vec::new(),
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
                tatoeba: SimpleToggleEngine::default(),
                baidu_web: SimpleToggleEngine::default(),
                caiyun_web: SimpleToggleEngine::default(),
                volcengine_web: SimpleToggleEngine::default(),
                transmart: SimpleToggleEngine::default(),
                papago: SimpleToggleEngine::default(),
            },
            default_from: "auto".into(),
            default_to: "zh".into(),
            custom_prompt: String::new(),
            prompt_templates: Vec::new(),
            clipboard_monitor: false,
            use_clipboard_output: true,
            auto_copy_result: false,
            auto_copy_mode: "translated".to_string(),
            translation_mask: false,
            api_server_enabled: false,
            api_server_port: 60828,
            api_server_token: String::new(),
            hotkeys: HotkeyConfig::default(),
            selection_ux: SelectionUxConfig::default(),
            proxy: ProxyConfig::default(),
            window_x: None,
            window_y: None,
            window_width: None,
            window_height: None,
            window_follow_mode: "none".to_string(),
            translation_blacklist: Vec::new(),
            routing_strategy: None,
            engine_order: Vec::new(),
            ocr_engine: default_ocr_engine(),
            offline_ocr: OfflineOcrConfig::default(),
            pdf_extraction_engine: default_pdf_extraction_engine(),
            pdf_extraction_sidecar: PdfExtractionSidecarConfig::default(),
            overlay_level: 2,
            overlay_auto_dismiss_ms: 3000,
            overlay_follow_mode: "none".to_string(),
            ocr_interval: 2000,
            ocr_click_through: false,
            ocr_auto_bind_window: true,
            hook: HookConfig::default(),
            tm_enabled: false,
            tm_threshold: 0.8,
            cache_ttl_hours: default_cache_ttl_hours(),
            furigana_enabled: false,
            tts_auto_play: false,
            tts_voice: String::new(),
            tts_provider: default_tts_provider(),
            batch_preferred_engine: String::new(),
            openai_tts: OpenAiTtsConfig::default(),
            fish_tts: FishTtsConfig::default(),
            http_timeout_secs: default_http_timeout_secs(),
            ocr_timeout_secs: default_ocr_timeout_secs(),
            llm_timeout_secs: default_llm_timeout_secs(),
            llm_temperature: default_llm_temperature(),
            llm_max_tokens: default_llm_max_tokens(),
            translation_timeout_secs: default_translation_timeout_secs(),
            edge_tts_token: default_edge_tts_token(),
            sync: SyncConfig::default(),
            collection: CollectionConfig::default(),
            layout_detection_enabled: false,
            winrt_ocr_use_subprocess: false,
            hot_load_page_count: default_hot_load_page_count(),
            defer_startup_warmup: default_defer_startup_warmup(),
        }
    }
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
        assert!(config.use_clipboard_output);
        assert!(!config.auto_copy_result);
        assert_eq!(config.auto_copy_mode, "translated");
        assert!(!config.translation_mask);
        assert!(!config.api_server_enabled);
        assert_eq!(config.api_server_port, 60828);
        assert!(config.api_server_token.is_empty());
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
        assert_eq!(config.cache_ttl_hours, 72);
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
        assert!(config.engines.youdao.ocr_app_key.is_empty());
        assert!(config.engines.youdao.ocr_app_secret.is_empty());
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
        assert_eq!(
            config.enabled_sources,
            vec!["uia".to_string(), "clipboard".to_string()]
        );
        assert!(!config.enabled_sources.contains(&"ocr".to_string()));
        assert!(!config.enabled_sources.contains(&"hook".to_string()));
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

    fn sample_llm() -> LlmConfig {
        LlmConfig {
            provider: "test".to_string(),
            api_key: String::new(),
            api_keys: Vec::new(),
            base_url: "https://api.example.com/v1".to_string(),
            model: "default-model".to_string(),
            providers: Vec::new(),
        }
    }

    fn sample_provider(
        id: &str,
        name: &str,
        priority: i32,
        enabled: bool,
        api_key: &str,
        base_url: &str,
        model: &str,
    ) -> LlmProviderEntry {
        LlmProviderEntry {
            id: id.to_string(),
            name: name.to_string(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            priority,
            enabled,
            models: Vec::new(),
            api_format: default_api_format(),
        }
    }

    #[test]
    fn test_llm_config_all_keys_empty() {
        let llm = sample_llm();
        assert!(llm.all_keys().is_empty());
    }

    #[test]
    fn test_llm_config_all_keys_single() {
        let mut llm = sample_llm();
        llm.api_key = "key1".to_string();
        assert_eq!(llm.all_keys(), vec!["key1"]);
    }

    #[test]
    fn test_llm_config_all_keys_multiple() {
        let mut llm = sample_llm();
        llm.api_key = "key1".to_string();
        llm.api_keys = vec!["key2".to_string(), "key3".to_string()];
        let keys = llm.all_keys();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0], "key1");
        assert_eq!(keys[1], "key2");
        assert_eq!(keys[2], "key3");
    }

    #[test]
    fn test_llm_config_all_keys_dedup() {
        let mut llm = sample_llm();
        llm.api_key = "key1".to_string();
        llm.api_keys = vec!["key1".to_string(), "key2".to_string()];
        let keys = llm.all_keys();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0], "key1");
        assert_eq!(keys[1], "key2");
    }

    #[test]
    fn test_llm_config_all_keys_skip_empty() {
        let mut llm = sample_llm();
        llm.api_keys = vec![String::new(), "key2".to_string(), String::new()];
        let keys = llm.all_keys();
        assert_eq!(keys, vec!["key2"]);
    }

    #[test]
    fn test_resolve_endpoints_empty_providers_fallback() {
        let mut llm = sample_llm();
        llm.provider = "deepseek".to_string();
        llm.api_key = "top-key".to_string();
        llm.base_url = "https://api.deepseek.com/v1".to_string();
        llm.model = "deepseek-chat".to_string();

        let endpoints = llm.resolve_endpoints();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(
            endpoints[0],
            LlmEndpoint {
                label: "deepseek".to_string(),
                api_key: "top-key".to_string(),
                base_url: "https://api.deepseek.com/v1".to_string(),
                model: "deepseek-chat".to_string(),
                api_format: "openai".to_string(),
            }
        );
    }

    #[test]
    fn test_resolve_endpoints_priority_sort() {
        let mut llm = sample_llm();
        llm.providers = vec![
            sample_provider(
                "b",
                "Second",
                20,
                true,
                "key-b",
                "https://b.example",
                "model-b",
            ),
            sample_provider(
                "a",
                "First",
                10,
                true,
                "key-a",
                "https://a.example",
                "model-a",
            ),
        ];

        let endpoints = llm.resolve_endpoints();
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].label, "First");
        assert_eq!(endpoints[0].api_key, "key-a");
        assert_eq!(endpoints[1].label, "Second");
        assert_eq!(endpoints[1].api_key, "key-b");
    }

    #[test]
    fn test_resolve_endpoints_skip_disabled_and_empty_key() {
        let mut llm = sample_llm();
        llm.api_key = "legacy".to_string();
        llm.providers = vec![
            sample_provider("off", "Disabled", 1, false, "key-off", "https://off", "m"),
            sample_provider("empty", "EmptyKey", 2, true, "", "https://empty", "m"),
            sample_provider("ok", "Ok", 3, true, "key-ok", "https://ok", "m-ok"),
        ];

        let endpoints = llm.resolve_endpoints();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].label, "Ok");
        assert_eq!(endpoints[0].api_key, "key-ok");
    }

    #[test]
    fn test_resolve_endpoints_empty_base_url_fallback() {
        let mut llm = sample_llm();
        llm.base_url = "https://fallback.example/v1".to_string();
        llm.model = "fallback-model".to_string();
        llm.providers = vec![sample_provider("p1", "Provider1", 1, true, "key-1", "", "")];

        let endpoints = llm.resolve_endpoints();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].base_url, "https://fallback.example/v1");
        assert_eq!(endpoints[0].model, "fallback-model");
        assert_eq!(endpoints[0].api_key, "key-1");
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

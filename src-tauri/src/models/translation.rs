use serde::{Deserialize, Serialize};

/// Result from a single translation engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub engine: String,
    pub text: String,
}

/// Response from translation containing results from one or more engines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResponse {
    pub results: Vec<TranslationResult>,
    pub detected_language: Option<String>,
}

/// Engine routing strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingStrategy {
    /// Use primary engine only, fail if it fails
    PrimaryOnly,
    /// Try primary, fallback to others on error
    FallbackOnError,
    /// Run all engines in parallel, return all results
    ParallelCompare,
    /// Prefer free engines, use paid only if all free fail
    CostAware,
    /// Use fastest engine based on historical latency
    LatencyFirst,
}

impl Default for RoutingStrategy {
    fn default() -> Self {
        Self::FallbackOnError
    }
}

/// Context from previous translations for document-level consistency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationContext {
    pub source: String,
    pub translation: String,
}

/// Translation mode determines how the translation is processed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TranslationMode {
    /// Single text translation using routing strategy
    Single,
    /// Primary engine only (quick translate)
    Primary,
    /// Streaming translation (for LLM engines)
    Stream,
    /// Batch translation (for documents, subtitles)
    Batch,
}

/// Unified translation job model
/// Captures all metadata for a translation request, used across all paths
/// (main translator, selection translate, subtitle, pdf, epub, API server)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationJob {
    /// Source text to translate
    pub text: String,
    /// Source language code (e.g., "auto", "en", "zh")
    pub from: String,
    /// Target language code (e.g., "zh", "en", "ja")
    pub to: String,
    /// Translation mode
    pub mode: TranslationMode,
    /// Optional context for document-level consistency
    pub context: Vec<TranslationContext>,
    /// Optional batch ID for grouping related translations
    pub batch_id: Option<String>,
    /// Concurrency for batch operations (default: 3)
    pub concurrency: usize,
}

impl Default for TranslationJob {
    fn default() -> Self {
        Self {
            text: String::new(),
            from: "auto".to_string(),
            to: "zh".to_string(),
            mode: TranslationMode::Single,
            context: Vec::new(),
            batch_id: None,
            concurrency: 3,
        }
    }
}

impl TranslationJob {
    /// Create a simple single-text translation job
    pub fn single(text: &str, from: &str, to: &str) -> Self {
        Self {
            text: text.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            mode: TranslationMode::Single,
            ..Default::default()
        }
    }

    /// Create a batch translation job for documents/subtitles
    pub fn batch(text: &str, from: &str, to: &str, concurrency: usize) -> Self {
        Self {
            text: text.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            mode: TranslationMode::Batch,
            concurrency: concurrency.max(1).min(10),
            ..Default::default()
        }
    }

    /// Add context for document-level consistency
    pub fn with_context(mut self, context: Vec<TranslationContext>) -> Self {
        self.context = context;
        self
    }

    /// Set batch ID for grouping related translations
    pub fn with_batch_id(mut self, batch_id: &str) -> Self {
        self.batch_id = Some(batch_id.to_string());
        self
    }
}

/// Result for a single line in batch translation
#[derive(Debug, Clone)]
pub struct BatchTranslationResult {
    pub index: usize,
    pub original: String,
    pub translated: String,
}

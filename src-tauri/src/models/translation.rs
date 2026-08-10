use serde::{Deserialize, Serialize};

/// Result from a single translation engine
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResult {
    pub engine: String,
    pub text: String,
    /// Optional latency in milliseconds (populated by `LatencyFirst` strategy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

/// Response from translation containing results from one or more engines
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateResponse {
    pub results: Vec<TranslationResult>,
    pub detected_language: Option<String>,
    /// Per-engine failure messages surfaced to the UI (empty on success).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl TranslateResponse {
    /// Overlay / selection card text: single engine plain; multi-engine `[name] text` lines.
    pub fn display_text(&self) -> String {
        let non_empty: Vec<&TranslationResult> = self
            .results
            .iter()
            .filter(|r| !r.text.trim().is_empty())
            .collect();
        if non_empty.is_empty() {
            return String::new();
        }
        if non_empty.len() == 1 {
            return non_empty[0].text.trim().to_string();
        }
        non_empty
            .iter()
            .map(|r| format!("[{}] {}", r.engine, r.text.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Engine routing strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RoutingStrategy {
    /// Use primary engine only, fail if it fails
    PrimaryOnly,
    /// Try primary, fallback to others on error
    #[default]
    FallbackOnError,
    /// Run all engines in parallel, return all results
    ParallelCompare,
    /// Prefer free engines, use paid only if all free fail
    CostAware,
    /// Use fastest engine based on historical latency
    LatencyFirst,
}


/// Context from previous translations for document-level consistency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationContext {
    pub source: String,
    pub translation: String,
}

/// Translation mode determines how the translation is processed
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranslationMode {
    /// Full pipeline + router strategy (multi-engine capable)
    #[default]
    #[serde(alias = "single")]
    Full,
    /// Primary engine only (quick translate / replace)
    Primary,
    /// Streaming translation (for LLM engines)
    Stream,
    /// Batch translation (for documents, subtitles)
    Batch,
    /// Parallel compare all engines (pipeline parity still open)
    Compare,
    /// Primary with document context segments
    Context,
    /// First-available engine result, resolved fast (floating card quick path;
    /// remaining engines load lazily on expand).
    Quick,
}

/// Product channel that initiated the request (metrics / policy later)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranslateChannel {
    #[default]
    Ui,
    Ocr,
    Selection,
    Replace,
    Hook,
    Clipboard,
    Document,
    Subtitle,
    Image,
    Http,
    Browser,
    Plugin,
    Unknown,
}

/// Unified façade request — all product paths should build this and call
/// `TranslationService::run` (or mode-specific helpers that delegate to it).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateRequest {
    pub channel: TranslateChannel,
    pub mode: TranslationMode,
    pub text: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub context: Vec<TranslationContext>,
    #[serde(default)]
    pub batch_id: Option<String>,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    /// Pre-split segments for batch mode; if empty, `text` is split by lines.
    #[serde(default)]
    pub segments: Vec<(usize, String)>,
    /// M4: per-region translate engine override ('' = global router primary).
    /// When set, the full/primary path uses this named engine instead of the
    /// router's primary. Mirrors `BatchConfig.engine` semantics (`translate_named`).
    #[serde(default)]
    pub engine: Option<String>,
}

fn default_concurrency() -> usize {
    3
}

impl Default for TranslateRequest {
    fn default() -> Self {
        Self {
            channel: TranslateChannel::Unknown,
            mode: TranslationMode::Full,
            text: String::new(),
            from: "auto".to_string(),
            to: "zh".to_string(),
            context: Vec::new(),
            batch_id: None,
            concurrency: 3,
            segments: Vec::new(),
            engine: None,
        }
    }
}

impl TranslateRequest {
    pub fn full(channel: TranslateChannel, text: impl Into<String>, from: &str, to: &str) -> Self {
        Self {
            channel,
            mode: TranslationMode::Full,
            text: text.into(),
            from: from.to_string(),
            to: to.to_string(),
            ..Default::default()
        }
    }

    pub fn primary(
        channel: TranslateChannel,
        text: impl Into<String>,
        from: &str,
        to: &str,
    ) -> Self {
        Self {
            channel,
            mode: TranslationMode::Primary,
            text: text.into(),
            from: from.to_string(),
            to: to.to_string(),
            ..Default::default()
        }
    }

    /// Quick mode: first-available engine result (floating card fast path).
    pub fn quick(
        channel: TranslateChannel,
        text: impl Into<String>,
        from: &str,
        to: &str,
    ) -> Self {
        Self {
            channel,
            mode: TranslationMode::Quick,
            text: text.into(),
            from: from.to_string(),
            to: to.to_string(),
            ..Default::default()
        }
    }

    pub fn batch(
        channel: TranslateChannel,
        text: impl Into<String>,
        from: &str,
        to: &str,
        concurrency: usize,
    ) -> Self {
        Self {
            channel,
            mode: TranslationMode::Batch,
            text: text.into(),
            from: from.to_string(),
            to: to.to_string(),
            concurrency: concurrency.max(1).min(10),
            ..Default::default()
        }
    }

    pub fn with_context(mut self, context: Vec<TranslationContext>) -> Self {
        self.context = context;
        self
    }

    pub fn with_batch_id(mut self, batch_id: &str) -> Self {
        self.batch_id = Some(batch_id.to_string());
        self
    }

    pub fn with_segments(mut self, segments: Vec<(usize, String)>) -> Self {
        self.segments = segments;
        self
    }
}

/// Outcome of `TranslationService::run` (non-stream).
#[derive(Debug, Clone)]
pub enum TranslateOutcome {
    Full(TranslateResponse),
    Primary(String),
    Batch(Vec<BatchTranslationResult>),
}

impl TranslateOutcome {
    pub fn into_full(self) -> Option<TranslateResponse> {
        match self {
            Self::Full(r) => Some(r),
            _ => None,
        }
    }

    pub fn into_primary(self) -> Option<String> {
        match self {
            Self::Primary(s) => Some(s),
            Self::Full(r) => r.results.into_iter().next().map(|x| x.text),
            _ => None,
        }
    }

    pub fn into_batch(self) -> Option<Vec<BatchTranslationResult>> {
        match self {
            Self::Batch(b) => Some(b),
            _ => None,
        }
    }
}

/// Legacy alias — prefer [`TranslateRequest`].
pub type TranslationJob = TranslateRequest;

impl TranslateRequest {
    /// Create a simple single-text translation job (legacy name)
    pub fn single(text: &str, from: &str, to: &str) -> Self {
        Self::full(TranslateChannel::Unknown, text, from, to)
    }
}

/// Result for a single line in batch translation
#[derive(Debug, Clone)]
pub struct BatchTranslationResult {
    pub index: usize,
    pub original: String,
    pub translated: String,
    /// Set when this segment failed (translated stays empty); lets callers
    /// distinguish "原文就是空" from "翻译失败" (M2-04).
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_response_serializes_detected_language_as_camel_case() {
        let response = TranslateResponse {
            results: vec![TranslationResult {
                engine: "test".to_string(),
                text: "你好".to_string(),
                latency_ms: None,
            }],
            detected_language: Some("en".to_string()),
            errors: vec![],
        };

        let json = serde_json::to_value(response).unwrap();

        assert_eq!(json["detectedLanguage"], "en");
        assert!(json.get("detected_language").is_none());
    }

    #[test]
    fn display_text_single_and_multi_engine() {
        let single = TranslateResponse {
            results: vec![TranslationResult {
                engine: "google".into(),
                text: "  你好  ".into(),
                latency_ms: None,
            }],
            detected_language: None,
            errors: vec![],
        };
        assert_eq!(single.display_text(), "你好");

        let multi = TranslateResponse {
            results: vec![
                TranslationResult {
                    engine: "google".into(),
                    text: "你好".into(),
                    latency_ms: None,
                },
                TranslationResult {
                    engine: "deepl".into(),
                    text: "您好".into(),
                    latency_ms: None,
                },
                TranslationResult {
                    engine: "empty".into(),
                    text: "  ".into(),
                    latency_ms: None,
                },
            ],
            detected_language: None,
            errors: vec![],
        };
        assert_eq!(multi.display_text(), "[google] 你好\n[deepl] 您好");
        assert!(TranslateResponse {
            results: vec![],
            detected_language: None,
            errors: vec![],
        }
        .display_text()
        .is_empty());
    }

    #[test]
    fn translate_request_full_builder() {
        let req = TranslateRequest::full(TranslateChannel::Ui, "hi", "en", "zh");
        assert_eq!(req.mode, TranslationMode::Full);
        assert_eq!(req.channel, TranslateChannel::Ui);
        assert_eq!(req.text, "hi");
    }

    #[test]
    fn translation_mode_deserializes_single_as_full() {
        let mode: TranslationMode = serde_json::from_str("\"single\"").unwrap();
        assert_eq!(mode, TranslationMode::Full);
    }
}

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::engine::TranslationEngine;

/// Caiyun Translate (彩云小译) translation engine
///
/// Features:
/// - Excellent context-aware translation (novels, articles)
/// - Free quota: 1M chars/month
/// - Supports: zh<->en, zh<->ja
///
/// API Documentation: https://docs.caiyunapp.com/blog/2018/09/03/lingocloud-api/
pub struct CaiyunEngine {
    api_token: String,
    client: Client,
}

#[derive(Debug, Serialize)]
struct CaiyunRequest {
    source: Vec<String>,
    trans_type: String,
    request_id: String,
    detect: bool,
}

#[derive(Debug, Deserialize)]
struct CaiyunResponse {
    target: Vec<String>,
    #[serde(default)]
    confidence: Option<f64>,
}

impl CaiyunEngine {
    pub fn new(api_token: &str) -> Self {
        Self {
            api_token: api_token.to_string(),
            client: Client::new(),
        }
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }

    /// Convert language code to Caiyun format
    ///
    /// Supported pairs:
    /// - auto2zh (auto detect to Chinese)
    /// - zh2en (Chinese to English)
    /// - en2zh (English to Chinese)
    /// - zh2ja (Chinese to Japanese)
    /// - ja2zh (Japanese to Chinese)
    fn convert_lang_pair(from: &str, to: &str) -> String {
        let from_code = match from {
            "auto" => "auto",
            "zh" | "zh-CN" | "zh-CHS" => "zh",
            "en" => "en",
            "ja" => "ja",
            _ => "auto",
        };

        let to_code = match to {
            "zh" | "zh-CN" | "zh-CHS" => "zh",
            "en" => "en",
            "ja" => "ja",
            _ => "zh",
        };

        format!("{}2{}", from_code, to_code)
    }
}

#[async_trait]
impl TranslationEngine for CaiyunEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        if text.is_empty() {
            return Ok(String::new());
        }

        let trans_type = Self::convert_lang_pair(from, to);
        let request_id = uuid::Uuid::new_v4().to_string();

        let payload = CaiyunRequest {
            source: vec![text.to_string()],
            trans_type,
            request_id,
            detect: from == "auto",
        };

        tracing::debug!("[Caiyun] Translating {} chars", text.len());

        let response = self
            .client
            .post("https://api.interpreter.caiyunai.com/v1/translator")
            .header("X-Authorization", format!("token {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("[Caiyun] API error: {} - {}", status, error_text);

            return Err(anyhow::anyhow!(
                "Caiyun API error: {} - {}",
                status,
                error_text
            ));
        }

        let result: CaiyunResponse = response.json().await?;

        let translated = result
            .target
            .first()
            .ok_or_else(|| anyhow::anyhow!("Empty response from Caiyun"))?
            .clone();

        if let Some(confidence) = result.confidence {
            tracing::debug!("[Caiyun] Translation confidence: {:.2}", confidence);
        }

        Ok(translated)
    }

    fn name(&self) -> &str {
        "Caiyun"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lang_pair_conversion() {
        assert_eq!(CaiyunEngine::convert_lang_pair("auto", "zh"), "auto2zh");
        assert_eq!(CaiyunEngine::convert_lang_pair("zh", "en"), "zh2en");
        assert_eq!(CaiyunEngine::convert_lang_pair("en", "zh"), "en2zh");
        assert_eq!(CaiyunEngine::convert_lang_pair("zh", "ja"), "zh2ja");
        assert_eq!(CaiyunEngine::convert_lang_pair("ja", "zh"), "ja2zh");
    }

    #[test]
    fn test_engine_creation() {
        let engine = CaiyunEngine::new("test_token");
        assert_eq!(engine.name(), "Caiyun");
    }
}

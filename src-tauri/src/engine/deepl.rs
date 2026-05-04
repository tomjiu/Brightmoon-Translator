use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct DeepLEngine {
    api_key: String,
    base_url: String,
    client: Client,
}

#[derive(Serialize)]
struct DeepLRequest {
    text: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_lang: Option<String>,
    target_lang: String,
}

#[derive(Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Deserialize)]
struct DeepLTranslation {
    text: String,
    #[serde(default)]
    detected_source_language: Option<String>,
}

impl DeepLEngine {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: "https://api-free.deepl.com/v2".to_string(),
            client: Client::new(),
        }
    }

    pub fn with_pro(mut self) -> Self {
        self.base_url = "https://api.deepl.com/v2".to_string();
        self
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }
}

#[async_trait]
impl TranslationEngine for DeepLEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let source_lang = if from == "auto" {
            None
        } else {
            // DeepL uses uppercase language codes
            let lang = from.to_uppercase();
            // Handle Chinese variants
            let lang = match lang.as_str() {
                "ZH" => "ZH".to_string(),
                "JA" => "JA".to_string(),
                "KO" => "KO".to_string(),
                _ => lang,
            };
            Some(lang)
        };

        // Map target language
        let target_lang = match to.to_uppercase().as_str() {
            "ZH" => "ZH".to_string(),
            "EN" => "EN-US".to_string(),
            "PT" => "PT-BR".to_string(),
            _ => to.to_uppercase(),
        };

        let request = DeepLRequest {
            text: vec![text.to_string()],
            source_lang,
            target_lang,
        };

        let url = format!("{}/translate", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("DeepL API error {}: {}", status, error_text));
        }

        let body: DeepLResponse = resp.json().await?;

        let translated = body
            .translations
            .first()
            .map(|t| t.text.clone())
            .unwrap_or_default();

        Ok(translated)
    }

    fn name(&self) -> &str {
        "DeepL"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

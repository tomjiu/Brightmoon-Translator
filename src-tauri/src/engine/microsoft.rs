use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

pub struct MicrosoftEngine {
    client: Client,
}

#[derive(Deserialize)]
struct MicrosoftResponse {
    translations: Vec<MicrosoftTranslation>,
}

#[derive(Deserialize)]
struct MicrosoftTranslation {
    text: String,
}

impl Default for MicrosoftEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MicrosoftEngine {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }
}

#[async_trait]
impl TranslationEngine for MicrosoftEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        // Use Bing Translator API (free, no key required)
        let _from_code = if from == "auto" { "" } else { from };
        let to_code = match to {
            "zh" => "zh-Hans",
            "ja" => "ja",
            "ko" => "ko",
            "fr" => "fr",
            "de" => "de",
            "es" => "es",
            "ru" => "ru",
            "pt" => "pt",
            "it" => "it",
            "ar" => "ar",
            "th" => "th",
            "vi" => "vi",
            _ => to,
        };

        let url = format!(
            "https://api.cognitive.microsofttranslator.com/translate?api-version=3.0&to={to_code}"
        );

        let body = serde_json::json!([{"Text": text}]);

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        super::check_response(&resp, "Microsoft")?;

        let result: Vec<MicrosoftResponse> = resp.json().await?;

        let translated = result
            .first()
            .and_then(|r| r.translations.first())
            .map(|t| t.text.clone())
            .ok_or_else(|| anyhow::anyhow!("Microsoft API returned empty translation result"))?;

        Ok(translated)
    }

    fn name(&self) -> &'static str {
        "Microsoft"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

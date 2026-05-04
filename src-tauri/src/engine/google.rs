use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;

pub struct GoogleEngine {
    client: Client,
}

impl GoogleEngine {
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
impl TranslationEngine for GoogleEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let from_code = if from == "auto" { "auto" } else { from };
        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
            from_code,
            to,
            urlencoding::encode(text)
        );

        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("Google API error: {}", status));
        }

        let body: serde_json::Value = resp.json().await?;

        // Parse the nested array response
        let translated = body[0]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item[0].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        Ok(translated)
    }

    fn name(&self) -> &str {
        "Google"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

pub struct YandexEngine {
    client: Client,
}

#[derive(Deserialize)]
struct YandexResponse {
    text: Vec<String>,
}

impl YandexEngine {
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
impl TranslationEngine for YandexEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        // Use Yandex Translate API (free web version)
        let lang = if from == "auto" {
            to.to_string()
        } else {
            format!("{}-{}", from, to)
        };

        let url = "https://translate.api.translator.net/yandex.json";

        let resp = self
            .client
            .post(url)
            .form(&[("lang", lang.as_str()), ("text", text)])
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("Yandex API error: {}", status));
        }

        let body: YandexResponse = resp.json().await?;

        let translated = body.text.join("");

        Ok(translated)
    }

    fn name(&self) -> &str {
        "Yandex"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

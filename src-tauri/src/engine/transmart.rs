//! Tencent TranSmart free API — pot transmart service.

use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;

pub struct TransmartEngine {
    client: Client,
}

impl TransmartEngine {
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

fn map_lang(code: &str) -> String {
    let c = code.trim().to_ascii_lowercase();
    match c.as_str() {
        "zh" | "zh-cn" | "zh-hans" => "zh".into(),
        "zh-tw" | "zh-hant" => "zh".into(),
        "auto" | "" => "auto".into(),
        other => other.to_string(),
    }
}

#[async_trait]
impl TranslationEngine for TransmartEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "header": { "fn": "auto_translation" },
            "type": "plain",
            "source": {
                "lang": map_lang(from),
                "text_list": [text],
            },
            "target": { "lang": map_lang(to) },
        });
        let resp = self
            .client
            .post("https://transmart.qq.com/api/imt")
            .header("Content-Type", "application/json")
            .header("User-Agent", "Mozilla/5.0")
            .json(&body)
            .send()
            .await?;
        super::check_response(&resp, "Transmart")?;
        let v: serde_json::Value = resp.json().await?;
        let lines = v
            .get("auto_translation")
            .and_then(|a| a.as_array())
            .ok_or_else(|| anyhow::anyhow!("Transmart: missing auto_translation: {v}"))?;
        let mut out = String::new();
        for line in lines {
            if let Some(s) = line.as_str() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(s);
            }
        }
        if out.trim().is_empty() {
            anyhow::bail!("Transmart: empty translation");
        }
        Ok(out.trim().to_string())
    }

    fn name(&self) -> &str {
        "TranSmart"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

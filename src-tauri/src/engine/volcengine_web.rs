//! Volcengine / 火山翻译 free web (CRX) endpoint — Luna huoshan.py.

use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;

pub struct VolcengineWebEngine {
    client: Client,
}

impl VolcengineWebEngine {
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
        "zh-tw" | "zh-hant" => "zh-Hant".into(),
        "auto" | "" => "auto".into(),
        other => other.to_string(),
    }
}

#[async_trait]
impl TranslationEngine for VolcengineWebEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let mut json_data = serde_json::json!({
            "text": text,
            "target_language": map_lang(to),
            "enable_user_glossary": false,
            "glossary_list": [],
            "category": "",
        });
        if from != "auto" && !from.is_empty() {
            json_data["source_language"] = serde_json::json!(map_lang(from));
        }

        let resp = self
            .client
            .post("https://translate.volcengine.com/crx/translate/v1/")
            .header("Accept", "application/json, text/plain, */*")
            .header(
                "Origin",
                "chrome-extension://klgfhbiooeogdfodpopgppeadghjjemk",
            )
            .header("User-Agent", "Mozilla/5.0")
            .json(&json_data)
            .send()
            .await?;
        super::check_response(&resp, "VolcengineWeb")?;
        let body: serde_json::Value = resp.json().await?;
        body.get("translation")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("VolcengineWeb: missing translation field: {body}"))
    }

    fn name(&self) -> &str {
        "Volcengine (free)"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

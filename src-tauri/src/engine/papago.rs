//! Naver Papago free web API — Luna papago.py.

use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;

pub struct PapagoEngine {
    client: Client,
}

impl PapagoEngine {
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
        "zh" | "zh-cn" | "zh-hans" => "zh-CN".into(),
        "zh-tw" | "zh-hant" => "zh-TW".into(),
        "auto" | "" => "auto".into(),
        "ja" => "ja".into(),
        "ko" => "ko".into(),
        "en" => "en".into(),
        other => other.to_string(),
    }
}

#[async_trait]
impl TranslationEngine for PapagoEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let resp = self
            .client
            .post("https://papago.naver.com/api/text/translation")
            .header("User-Agent", "Mozilla/5.0")
            .form(&[
                ("dict", "true"),
                ("dictDisplay", "30"),
                ("honorific", "false"),
                ("useGlossary", "false"),
                ("source", &map_lang(from)),
                ("target", &map_lang(to)),
                ("text", text),
            ])
            .send()
            .await?;
        super::check_response(&resp, "Papago")?;
        let v: serde_json::Value = resp.json().await?;
        v.get("translatedText")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Papago: missing translatedText: {v}"))
    }

    fn name(&self) -> &str {
        "Papago"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

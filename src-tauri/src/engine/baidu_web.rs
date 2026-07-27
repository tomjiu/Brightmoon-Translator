//! Baidu free web translate via public sug/v2transapi-style fallback.
//! Uses the mobile-friendly endpoint that does not require appid.
//! Marked unofficial — may break.

use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;

pub struct BaiduWebEngine {
    client: Client,
}

impl BaiduWebEngine {
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
        "zh-tw" | "zh-hant" => "cht".into(),
        "auto" | "" => "auto".into(),
        "ja" => "jp".into(),
        "ko" => "kor".into(),
        "fr" => "fra".into(),
        "es" => "spa".into(),
        other => other.to_string(),
    }
}

#[async_trait]
impl TranslationEngine for BaiduWebEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        // Public langdetect + v2transapi without signed token often fails;
        // use fanyi-api free guest path used by many open-source clients:
        // https://fanyi.baidu.com/ait/text/translate (JSON stream) is heavy.
        // Fallback: Microsoft-style is already free; here use simple
        // https://api.m.baidu.com/sdktest/translate or the older:
        let url = format!(
            "https://fanyi.baidu.com/transapi?from={}&to={}&query={}",
            map_lang(from),
            map_lang(to),
            urlencoding::encode(text)
        );
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .header("Referer", "https://fanyi.baidu.com/")
            .send()
            .await?;
        super::check_response(&resp, "BaiduWeb")?;
        let body: serde_json::Value = resp.json().await?;

        // Format A: { "data": [ { "dst": "..." } ] }
        if let Some(arr) = body.get("data").and_then(|d| d.as_array()) {
            let mut out = String::new();
            for item in arr {
                if let Some(dst) = item.get("dst").and_then(|d| d.as_str()) {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(dst);
                }
            }
            if !out.is_empty() {
                return Ok(out);
            }
        }
        // Format B: { "result": "..." } or result as JSON string
        if let Some(r) = body.get("result").and_then(|r| r.as_str()) {
            if let Ok(inner) = serde_json::from_str::<serde_json::Value>(r) {
                if let Some(arr) = inner
                    .pointer("/trans_result/data")
                    .and_then(|d| d.as_array())
                {
                    let mut out = String::new();
                    for item in arr {
                        if let Some(dst) = item.get("dst").and_then(|d| d.as_str()) {
                            if !out.is_empty() {
                                out.push('\n');
                            }
                            out.push_str(dst);
                        }
                    }
                    if !out.is_empty() {
                        return Ok(out);
                    }
                }
            }
            if !r.is_empty() {
                return Ok(r.to_string());
            }
        }
        anyhow::bail!("BaiduWeb: unexpected response {body}")
    }

    fn name(&self) -> &str {
        "Baidu (free web)"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

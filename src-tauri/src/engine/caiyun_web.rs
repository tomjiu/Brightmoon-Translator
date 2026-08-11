//! Caiyun free web path (JWT + browser token) — Luna caiyun.py simplified.

use super::TranslationEngine;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use reqwest::Client;

const WEB_TOKEN: &str = "token:qgemv4jr1y38jyq6vhvi";
const BROWSER_ID: &str = "beba19f9d7f10c74c98334c9e8afcd34";

pub struct CaiyunWebEngine {
    client: Client,
}

impl Default for CaiyunWebEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CaiyunWebEngine {
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
        "zh" | "zh-cn" | "zh-hans" | "zh-tw" | "zh-hant" => "zh".into(),
        "auto" | "" => "auto".into(),
        other => other.to_string(),
    }
}

fn crypt_map(decrypt: bool) -> std::collections::HashMap<char, char> {
    let normal: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789=.+-_/"
        .chars()
        .collect();
    let cipher: Vec<char> = "NOPQRSTUVWXYZABCDEFGHIJKLMnopqrstuvwxyzabcdefghijklm0123456789=.+-_/"
        .chars()
        .collect();
    if decrypt {
        cipher.into_iter().zip(normal).collect()
    } else {
        normal.into_iter().zip(cipher).collect()
    }
}

fn decrypt_target(cipher_text: &str) -> anyhow::Result<String> {
    let map = crypt_map(true);
    let mapped: String = cipher_text
        .chars()
        .map(|c| *map.get(&c).unwrap_or(&c))
        .collect();
    let bytes = B64
        .decode(mapped.as_bytes())
        .map_err(|e| anyhow::anyhow!("CaiyunWeb decrypt base64: {e}"))?;
    String::from_utf8(bytes).map_err(|e| anyhow::anyhow!("CaiyunWeb utf8: {e}"))
}

#[async_trait]
impl TranslationEngine for CaiyunWebEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let headers_common = [
            ("authority", "api.interpreter.caiyunai.com"),
            ("accept", "application/json, text/plain, */*"),
            ("app-name", "xy"),
            ("origin", "https://fanyi.caiyunapp.com"),
            ("os-type", "web"),
            ("referer", "https://fanyi.caiyunapp.com/"),
            ("x-authorization", WEB_TOKEN),
            ("User-Agent", "Mozilla/5.0"),
        ];

        let jwt_resp = self
            .client
            .post("https://api.interpreter.caiyunai.com/v1/user/jwt/generate")
            .headers({
                let mut h = reqwest::header::HeaderMap::new();
                for (k, v) in &headers_common {
                    if let (Ok(name), Ok(val)) = (
                        reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                        reqwest::header::HeaderValue::from_str(v),
                    ) {
                        h.insert(name, val);
                    }
                }
                h
            })
            .json(&serde_json::json!({ "browser_id": BROWSER_ID }))
            .send()
            .await?;
        super::check_response(&jwt_resp, "CaiyunWeb-jwt")?;
        let jwt_body: serde_json::Value = jwt_resp.json().await?;
        let jwt = jwt_body
            .get("jwt")
            .and_then(|j| j.as_str())
            .ok_or_else(|| anyhow::anyhow!("CaiyunWeb: no jwt"))?;

        let trans_type = format!("{}2{}", map_lang(from), map_lang(to));
        let resp = self
            .client
            .post("https://api.interpreter.caiyunai.com/v1/translator")
            .header("Content-Type", "application/json")
            .header("x-authorization", WEB_TOKEN)
            .header("t-authorization", jwt)
            .header("app-name", "xy")
            .header("os-type", "web")
            .header("origin", "https://fanyi.caiyunapp.com")
            .header("referer", "https://fanyi.caiyunapp.com/")
            .header("User-Agent", "Mozilla/5.0")
            .json(&serde_json::json!({
                "source": text,
                "trans_type": trans_type,
                "request_id": "web_fanyi",
                "media": "text",
                "os_type": "web",
                "dict": true,
                "cached": true,
                "replaced": true,
                "detect": true,
                "browser_id": BROWSER_ID,
            }))
            .send()
            .await?;
        super::check_response(&resp, "CaiyunWeb")?;
        let body: serde_json::Value = resp.json().await?;
        let target = body
            .get("target")
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow::anyhow!("CaiyunWeb: missing target: {body}"))?;
        decrypt_target(target)
    }

    fn name(&self) -> &'static str {
        "Caiyun (free web)"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

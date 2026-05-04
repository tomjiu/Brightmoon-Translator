use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub struct BaiduEngine {
    app_id: String,
    secret: String,
    client: Client,
}

#[derive(Deserialize)]
struct BaiduResponse {
    trans_result: Option<Vec<BaiduTransResult>>,
    error_code: Option<String>,
    error_msg: Option<String>,
}

#[derive(Deserialize)]
struct BaiduTransResult {
    dst: String,
}

impl BaiduEngine {
    pub fn new(app_id: &str, secret: &str) -> Self {
        Self {
            app_id: app_id.to_string(),
            secret: secret.to_string(),
            client: Client::new(),
        }
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }
}

#[async_trait]
impl TranslationEngine for BaiduEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let salt = uuid::Uuid::new_v4().to_string();
        let sign_input = format!("{}{}{}{}", self.app_id, text, salt, self.secret);
        let mut hasher = Sha256::new();
        hasher.update(sign_input.as_bytes());
        let sign = format!("{:x}", hasher.finalize());

        let params = [
            ("q", text),
            ("from", from),
            ("to", to),
            ("appid", &self.app_id),
            ("salt", &salt),
            ("sign", &sign),
        ];

        let resp = self
            .client
            .post("https://fanyi-api.baidu.com/api/trans/vip/translate")
            .form(&params)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("Baidu API error: {}", status));
        }

        let body: BaiduResponse = resp.json().await?;

        if let Some(code) = body.error_code {
            return Err(anyhow::anyhow!("Baidu error {}: {}", code, body.error_msg.unwrap_or_default()));
        }

        let translated = body
            .trans_result
            .unwrap_or_default()
            .iter()
            .map(|r| r.dst.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(translated)
    }

    fn name(&self) -> &str {
        "Baidu"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use rand::Rng;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

/// DeepLX - Built-in DeepL free API implementation
/// Based on DeepLX algorithm, no external service needed
pub struct DeepLXEngine {
    client: Client,
    use_pro: bool,
    api_key: Option<String>,
    max_retries: u32,
}

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    id: u64,
    params: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    result: Option<TranslateResult>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct TranslateResult {
    #[serde(default)]
    translations: Option<Vec<Translation>>,
    #[serde(default)]
    source_lang: Option<String>,
    #[serde(default)]
    target_lang: Option<String>,
}

#[derive(Deserialize)]
struct Translation {
    #[serde(default)]
    beams: Option<Vec<Beam>>,
}

#[derive(Deserialize)]
struct Beam {
    #[serde(default)]
    sentences: Option<Vec<Sentence>>,
}

#[derive(Deserialize)]
struct Sentence {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct RpcError {
    #[serde(default)]
    code: Option<i64>,
    #[serde(default)]
    message: Option<String>,
}

impl DeepLXEngine {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            use_pro: false,
            api_key: None,
            max_retries: 3,
        }
    }

    pub fn with_pro(mut self, pro: bool) -> Self {
        self.use_pro = pro;
        self
    }

    pub fn with_api_key(mut self, key: &str) -> Self {
        self.api_key = Some(key.to_string());
        self
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }

    fn map_lang(lang: &str) -> &str {
        match lang.to_uppercase().as_str() {
            "AUTO" => "auto",
            "ZH" => "ZH",
            "EN" => "EN",
            "JA" => "JA",
            "KO" => "KO",
            "FR" => "FR",
            "DE" => "DE",
            "ES" => "ES",
            "RU" => "RU",
            "PT" => "PT",
            "IT" => "IT",
            "AR" => "AR",
            "NL" => "NL",
            "PL" => "PL",
            "SV" => "SV",
            "DA" => "DA",
            "FI" => "FI",
            "EL" => "EL",
            "CS" => "CS",
            "RO" => "RO",
            "HU" => "HU",
            "TR" => "TR",
            "UK" => "UK",
            "ID" => "ID",
            _ => lang,
        }
    }

    /// Get count of 'i' characters in text (DeepLX uses this for timestamp)
    fn get_i_count(text: &str) -> i64 {
        text.chars().filter(|&c| c == 'i').count() as i64
    }

    /// Generate random ID matching DeepLX format: (100000..199998) * 1000
    fn generate_id() -> i64 {
        let mut rng = rand::thread_rng();
        let base = rng.gen_range(100000..199999);
        base * 1000
    }

    /// Generate timestamp based on iCount (DeepLX algorithm)
    fn get_timestamp(i_count: i64) -> i64 {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        if i_count != 0 {
            let i_count = i_count + 1;
            ts - (ts % i_count) + i_count
        } else {
            ts
        }
    }

    /// Manipulate request body like DeepLX does
    fn handler_body_method(random_id: i64, body: String) -> String {
        let calc = (random_id + 5) % 29 == 0 || (random_id + 3) % 13 == 0;
        if calc {
            body.replacen("\"method\":\"", "\"method\" : \"", 1)
        } else {
            body.replacen("\"method\":\"", "\"method\": \"", 1)
        }
    }

    async fn translate_free(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let source_lang = Self::map_lang(from);
        let target_lang = Self::map_lang(to);

        let mut last_error = None;

        // Retry logic with exponential backoff
        for attempt in 0..self.max_retries {
            if attempt > 0 {
                // Exponential backoff: 2s, 4s, 8s...
                let delay = Duration::from_secs(2u64.pow(attempt));
                sleep(delay).await;
            }

            let id = Self::generate_id();
            let i_count = Self::get_i_count(text);
            let timestamp = Self::get_timestamp(i_count);

            // Add small random delay to avoid detection
            let jitter = rand::thread_rng().gen_range(100..500);
            sleep(Duration::from_millis(jitter)).await;

            // Build request matching DeepLX format
            let post_data = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "LMT_handle_texts",
                "id": id,
                "params": {
                    "splitting": "newlines",
                    "lang": {
                        "source_lang_user_selected": source_lang,
                        "target_lang": target_lang
                    },
                    "texts": [{
                        "text": text,
                        "requestAlternatives": 3
                    }],
                    "timestamp": timestamp
                }
            });

            // Format body and apply manipulation like DeepLX
            let post_str = post_data.to_string();
            let post_str = Self::handler_body_method(id, post_str);

            let resp = match self
                .client
                .post("https://www2.deepl.com/jsonrpc")
                .header("Content-Type", "application/json")
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header("Origin", "https://www.deepl.com")
                .header("Referer", "https://www.deepl.com/")
                .header("Sec-Fetch-Dest", "empty")
                .header("Sec-Fetch-Mode", "cors")
                .header("Sec-Fetch-Site", "same-site")
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0")
                .header("Content-Length", post_str.len().to_string())
                .body(post_str)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(format!("Request failed: {}", e));
                    continue;
                }
            };

            let status = resp.status();

            // Rate limited - retry after backoff
            if status == 429 {
                last_error = Some("Rate limited, retrying...".to_string());
                continue;
            }

            if !status.is_success() {
                let error_text = resp.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!("DeepL API error {}: {}", status, error_text));
            }

            let body: JsonRpcResponse = resp.json().await?;

            if let Some(error) = body.error {
                // Rate limit error code
                if error.code == Some(1042911) {
                    last_error = Some("Rate limited, retrying...".to_string());
                    continue;
                }
                let msg = error.message.unwrap_or_else(|| "Unknown error".to_string());
                return Err(anyhow::anyhow!("DeepL RPC error: {}", msg));
            }

            if let Some(result) = body.result {
                if let Some(translations) = result.translations {
                    let mut texts = Vec::new();
                    for translation in &translations {
                        if let Some(beams) = &translation.beams {
                            if let Some(first_beam) = beams.first() {
                                if let Some(sentences) = &first_beam.sentences {
                                    for sentence in sentences {
                                        if let Some(text) = &sentence.text {
                                            texts.push(text.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !texts.is_empty() {
                        return Ok(texts.join(""));
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "DeepL rate limited after {} retries. {}",
            self.max_retries,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        ))
    }

    async fn translate_pro(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let api_key = self.api_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("DeepL Pro API key not configured"))?;

        let source_lang = Self::map_lang(from);
        let target_lang = Self::map_lang(to);

        let params = serde_json::json!({
            "text": [text],
            "target_lang": target_lang,
            "source_lang": if source_lang == "auto" { serde_json::Value::Null } else { serde_json::Value::String(source_lang.to_string()) }
        });

        let base_url = if self.use_pro {
            "https://api.deepl.com/v2/translate"
        } else {
            "https://api-free.deepl.com/v2/translate"
        };

        let resp = self
            .client
            .post(base_url)
            .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
            .header("Content-Type", "application/json")
            .json(&params)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("DeepL Pro API error {}: {}", status, error_text));
        }

        let body: serde_json::Value = resp.json().await?;

        if let Some(translations) = body.get("translations").and_then(|t| t.as_array()) {
            if let Some(first) = translations.first() {
                if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                    return Ok(text.to_string());
                }
            }
        }

        Err(anyhow::anyhow!("DeepL Pro returned empty response"))
    }
}

#[async_trait]
impl TranslationEngine for DeepLXEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        // If API key is provided, use Pro API; otherwise use free API
        if self.api_key.is_some() {
            self.translate_pro(text, from, to).await
        } else {
            self.translate_free(text, from, to).await
        }
    }

    fn name(&self) -> &str {
        "DeepLX"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

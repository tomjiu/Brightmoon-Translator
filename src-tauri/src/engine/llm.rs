use super::TranslationEngine;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;

pub struct LlmEngine {
    api_keys: Vec<String>,
    base_url: String,
    model: String,
    custom_prompt: String,
    client: Client,
    key_index: AtomicUsize,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[derive(Deserialize)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

impl LlmEngine {
    pub fn new(api_key: &str, base_url: &str, model: &str) -> Self {
        let mut keys = Vec::new();
        if !api_key.is_empty() {
            keys.push(api_key.to_string());
        }
        Self {
            api_keys: keys,
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            custom_prompt: String::new(),
            client: Client::new(),
            key_index: AtomicUsize::new(0),
        }
    }

    pub fn with_multiple_keys(api_keys: Vec<String>, base_url: &str, model: &str) -> Self {
        Self {
            api_keys,
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            custom_prompt: String::new(),
            client: Client::new(),
            key_index: AtomicUsize::new(0),
        }
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }

    pub fn with_custom_prompt(mut self, prompt: &str) -> Self {
        self.custom_prompt = prompt.to_string();
        self
    }

    /// Get next API key using round-robin rotation
    fn next_key(&self) -> Option<&str> {
        if self.api_keys.is_empty() {
            return None;
        }
        let idx = self.key_index.fetch_add(1, Ordering::Relaxed) % self.api_keys.len();
        Some(&self.api_keys[idx])
    }

    fn build_system_prompt(&self, from: &str, to: &str) -> String {
        let lang_map = |code: &str| -> String {
            match code {
                "zh" => "中文".to_string(),
                "en" => "English".to_string(),
                "ja" => "日本語".to_string(),
                "ko" => "한국어".to_string(),
                "fr" => "Français".to_string(),
                "de" => "Deutsch".to_string(),
                "es" => "Español".to_string(),
                "ru" => "Русский".to_string(),
                "pt" => "Português".to_string(),
                "it" => "Italiano".to_string(),
                "ar" => "العربية".to_string(),
                "th" => "ไทย".to_string(),
                "vi" => "Tiếng Việt".to_string(),
                "auto" => "自动检测".to_string(),
                _ => code.to_string(),
            }
        };

        let from_lang = lang_map(from);
        let to_lang = lang_map(to);

        if !self.custom_prompt.is_empty() {
            self.custom_prompt
                .replace("{from}", &from_lang)
                .replace("{to}", &to_lang)
                .replace("{source_lang}", &from_lang)
                .replace("{target_lang}", &to_lang)
        } else {
            format!(
                r#"你是一个专业的翻译专家。请遵循以下规则：
1. 准确传达原文含义，保持自然流畅
2. 专业术语使用标准译法
3. 保持原文的语气和风格
4. 对于代码/技术内容，保留原文格式
5. 只返回翻译结果，不要添加任何解释或前缀

源语言：{from_lang}
目标语言：{to_lang}"#
            )
        }
    }

    pub async fn translate_stream(
        &self,
        text: &str,
        from: &str,
        to: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        let system_prompt = self.build_system_prompt(from, to);
        let total_keys = self.api_keys.len();

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                Message {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            temperature: 0.3,
            max_tokens: 4096,
            stream: true,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let mut last_error = String::new();

        // Try each key on failure
        for attempt in 0..total_keys.max(1) {
            let key = if total_keys > 0 {
                let idx = (self.key_index.fetch_add(1, Ordering::Relaxed)) % total_keys;
                &self.api_keys[idx]
            } else {
                ""
            };

            let mut req = self.client.post(&url).json(&request);
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        let body = resp.text().await.unwrap_or_default();
                        last_error = format!("LLM API error {}: {}", status, body);
                        eprintln!("Key attempt {} failed: {}", attempt + 1, last_error);
                        continue;
                    }

                    let mut stream = resp.bytes_stream();
                    let mut full_text = String::new();
                    let mut buffer = String::new();

                    while let Some(chunk) = stream.next().await {
                        let chunk = chunk?;
                        let text = String::from_utf8_lossy(&chunk);
                        buffer.push_str(&text);

                        while let Some(line_end) = buffer.find('\n') {
                            let line = buffer[..line_end].trim().to_string();
                            buffer = buffer[line_end + 1..].to_string();

                            if line.starts_with("data: ") {
                                let data = &line[6..];
                                if data == "[DONE]" {
                                    break;
                                }

                                if let Ok(resp) = serde_json::from_str::<StreamResponse>(data) {
                                    if let Some(choice) = resp.choices.first() {
                                        if let Some(content) = &choice.delta.content {
                                            full_text.push_str(content);
                                            let _ = tx.send(content.clone()).await;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    return Ok(full_text);
                }
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    eprintln!("Key attempt {} failed: {}", attempt + 1, last_error);
                    continue;
                }
            }
        }

        Err(anyhow::anyhow!(
            "All {} API keys failed. Last error: {}",
            total_keys,
            last_error
        ))
    }
}

#[async_trait]
impl TranslationEngine for LlmEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let system_prompt = self.build_system_prompt(from, to);
        let total_keys = self.api_keys.len();

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                Message {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            temperature: 0.3,
            max_tokens: 4096,
            stream: false,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let mut last_error = String::new();

        // Try each key on failure
        for attempt in 0..total_keys.max(1) {
            let key = if total_keys > 0 {
                let idx = (self.key_index.fetch_add(1, Ordering::Relaxed)) % total_keys;
                &self.api_keys[idx]
            } else {
                ""
            };

            let mut req = self.client.post(&url).json(&request);
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        let body = resp.text().await.unwrap_or_default();
                        last_error = format!("LLM API error {}: {}", status, body);
                        eprintln!("Key attempt {} failed: {}", attempt + 1, last_error);
                        continue;
                    }

                    let chat_resp: ChatResponse = resp.json().await?;
                    let content = chat_resp
                        .choices
                        .first()
                        .map(|c| c.message.content.trim().to_string())
                        .unwrap_or_default();

                    return Ok(content);
                }
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    eprintln!("Key attempt {} failed: {}", attempt + 1, last_error);
                    continue;
                }
            }
        }

        Err(anyhow::anyhow!(
            "All {} API keys failed. Last error: {}",
            total_keys,
            last_error
        ))
    }

    fn name(&self) -> &str {
        "LLM"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Context pair for maintaining translation consistency
#[derive(Debug, Clone)]
pub struct TranslationContext {
    pub source: String,
    pub translation: String,
}

impl LlmEngine {
    /// Translate with context from previous translations for consistency
    pub async fn translate_with_context(
        &self,
        text: &str,
        from: &str,
        to: &str,
        context: &[TranslationContext],
    ) -> anyhow::Result<String> {
        if context.is_empty() {
            return self.translate(text, from, to).await;
        }

        let system_prompt = self.build_system_prompt(from, to);
        let total_keys = self.api_keys.len();

        // Build context message
        let mut context_lines = Vec::new();
        context_lines.push("以下是之前的翻译参考，请保持术语和风格一致：".to_string());
        for (i, ctx) in context.iter().enumerate().take(5) {
            context_lines.push(format!(
                "{}. \"{}\" → \"{}\"",
                i + 1,
                truncate_text(&ctx.source, 100),
                truncate_text(&ctx.translation, 100)
            ));
        }

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                Message {
                    role: "user".to_string(),
                    content: context_lines.join("\n"),
                },
                Message {
                    role: "assistant".to_string(),
                    content: "好的，我会参考之前的翻译保持一致性。".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            temperature: 0.3,
            max_tokens: 4096,
            stream: false,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let mut last_error = String::new();

        for attempt in 0..total_keys.max(1) {
            let key = if total_keys > 0 {
                let idx = (self.key_index.fetch_add(1, Ordering::Relaxed)) % total_keys;
                &self.api_keys[idx]
            } else {
                ""
            };

            let mut req = self.client.post(&url).json(&request);
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        let body = resp.text().await.unwrap_or_default();
                        last_error = format!("LLM API error {}: {}", status, body);
                        eprintln!("Key attempt {} failed: {}", attempt + 1, last_error);
                        continue;
                    }

                    let chat_resp: ChatResponse = resp.json().await?;
                    let content = chat_resp
                        .choices
                        .first()
                        .map(|c| c.message.content.trim().to_string())
                        .unwrap_or_default();

                    return Ok(content);
                }
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    eprintln!("Key attempt {} failed: {}", attempt + 1, last_error);
                    continue;
                }
            }
        }

        Err(anyhow::anyhow!(
            "All {} API keys failed. Last error: {}",
            total_keys,
            last_error
        ))
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

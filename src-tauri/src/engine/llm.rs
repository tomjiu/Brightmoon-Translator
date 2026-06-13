use super::TranslationEngine;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;

/// Sanitize LLM error messages before sending to frontend.
/// Uses the centralized sanitization from the security module to strip
/// API keys, tokens, and other sensitive patterns.
fn sanitize_llm_error(status: reqwest::StatusCode, body: &str) -> String {
    // Truncate long error bodies (use chars to avoid UTF-8 boundary issues)
    let truncated: String = body.chars().take(200).collect();
    // Use centralized sanitization
    let sanitized = crate::security::sanitize_log_message(&truncated);
    format!("LLM API error {}: {}", status, sanitized)
}

pub struct LlmEngine {
    api_keys: Vec<String>,
    base_url: String,
    model: String,
    custom_prompt: String,
    temperature: f32,
    max_tokens: u32,
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
            temperature: 0.3,
            max_tokens: 4096,
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
            temperature: 0.3,
            max_tokens: 4096,
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

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp.clamp(0.0, 2.0);
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    fn build_system_prompt(&self, from: &str, to: &str, glossary_hint: Option<&str>) -> String {
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

        let base = if !self.custom_prompt.is_empty() {
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
        };

        // Append glossary hint if available
        match glossary_hint {
            Some(hint) if !hint.is_empty() => format!("{}\n\n{}", base, hint),
            _ => base,
        }
    }

    /// Build a standard 2-message chat request (system + user)
    fn build_request(&self, system_prompt: &str, user_text: &str, stream: bool) -> ChatRequest {
        ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_text.to_string(),
                },
            ],
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream,
        }
    }

    /// Shared non-streaming LLM call logic with key rotation.
    /// Accepts pre-built messages so callers can customize the conversation.
    async fn call_llm_with_messages(&self, messages: Vec<Message>) -> anyhow::Result<String> {
        let total_keys = self.api_keys.len();
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
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
                        tracing::warn!(
                            "Key attempt {} failed: LLM API error {}: {}",
                            attempt + 1,
                            status,
                            crate::security::sanitize_log_message(&body)
                        );
                        let sanitized = sanitize_llm_error(status, &body);
                        last_error = sanitized;
                        continue;
                    }

                    let chat_resp: ChatResponse = resp.json().await?;
                    let content = chat_resp
                        .choices
                        .first()
                        .map(|c| c.message.content.trim().to_string())
                        .ok_or_else(|| {
                            anyhow::anyhow!("LLM API returned no choices in response")
                        })?;

                    return Ok(content);
                },
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    tracing::warn!("Key attempt {} failed: {}", attempt + 1, last_error);
                    continue;
                },
            }
        }

        Err(anyhow::anyhow!(
            "All {} API keys failed. Last error: {}",
            total_keys,
            last_error
        ))
    }

    /// Non-streaming LLM call with system_prompt and user_text convenience wrapper
    async fn call_llm(&self, system_prompt: &str, user_text: &str) -> anyhow::Result<String> {
        let messages = self.build_request(system_prompt, user_text, false).messages;
        self.call_llm_with_messages(messages).await
    }

    /// Non-streaming LLM call with a custom temperature value.
    async fn call_llm_with_temperature(
        &self,
        system_prompt: &str,
        user_text: &str,
        temperature: f32,
    ) -> anyhow::Result<String> {
        let total_keys = self.api_keys.len();
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_text.to_string(),
                },
            ],
            temperature,
            max_tokens: self.max_tokens,
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
                        tracing::warn!(
                            "Key attempt {} failed: LLM API error {}: {}",
                            attempt + 1,
                            status,
                            crate::security::sanitize_log_message(&body)
                        );
                        let sanitized = sanitize_llm_error(status, &body);
                        last_error = sanitized;
                        continue;
                    }

                    let chat_resp: ChatResponse = resp.json().await?;
                    let content = chat_resp
                        .choices
                        .first()
                        .map(|c| c.message.content.trim().to_string())
                        .ok_or_else(|| {
                            anyhow::anyhow!("LLM API returned no choices in response")
                        })?;

                    return Ok(content);
                },
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    tracing::warn!("Key attempt {} failed: {}", attempt + 1, last_error);
                    continue;
                },
            }
        }

        Err(anyhow::anyhow!(
            "All {} API keys failed. Last error: {}",
            total_keys,
            last_error
        ))
    }

    /// Shared streaming LLM call logic with key rotation.
    /// Accepts pre-built messages so callers can customize the conversation.
    async fn stream_llm(
        &self,
        messages: Vec<Message>,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        let total_keys = self.api_keys.len();
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: true,
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
                        tracing::warn!(
                            "Key attempt {} failed: LLM API error {}: {}",
                            attempt + 1,
                            status,
                            crate::security::sanitize_log_message(&body)
                        );
                        let sanitized = sanitize_llm_error(status, &body);
                        last_error = sanitized;
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
                            // Use drain() to avoid allocating a new String for the remaining buffer
                            let line: String = buffer.drain(..=line_end).collect();
                            let line = line.trim();

                            if let Some(data) = line.strip_prefix("data: ") {
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
                },
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    tracing::warn!("Key attempt {} failed: {}", attempt + 1, last_error);
                    continue;
                },
            }
        }

        Err(anyhow::anyhow!(
            "All {} API keys failed. Last error: {}",
            total_keys,
            last_error
        ))
    }

    pub async fn translate_stream(
        &self,
        text: &str,
        from: &str,
        to: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        let system_prompt = self.build_system_prompt(from, to, None);
        let messages = self.build_request(&system_prompt, text, true).messages;
        self.stream_llm(messages, tx).await
    }

    /// Stream translation with glossary hint injected into the system prompt
    pub async fn translate_stream_with_glossary(
        &self,
        text: &str,
        from: &str,
        to: &str,
        tx: mpsc::Sender<String>,
        glossary_hint: &str,
    ) -> anyhow::Result<String> {
        if glossary_hint.is_empty() {
            return self.translate_stream(text, from, to, tx).await;
        }
        let system_prompt = self.build_system_prompt(from, to, Some(glossary_hint));
        let messages = self.build_request(&system_prompt, text, true).messages;
        self.stream_llm(messages, tx).await
    }
}

#[async_trait]
impl TranslationEngine for LlmEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let system_prompt = self.build_system_prompt(from, to, None);
        self.call_llm(&system_prompt, text).await
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

        let system_prompt = self.build_system_prompt(from, to, None);

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

        let messages = vec![
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
        ];

        self.call_llm_with_messages(messages).await
    }

    /// Translate with glossary terms injected into the system prompt.
    /// This tells the LLM about preferred translations for specific terms.
    pub async fn translate_with_glossary(
        &self,
        text: &str,
        from: &str,
        to: &str,
        glossary_hint: &str,
    ) -> anyhow::Result<String> {
        if glossary_hint.is_empty() {
            return self.translate(text, from, to).await;
        }
        let system_prompt = self.build_system_prompt(from, to, Some(glossary_hint));
        self.call_llm(&system_prompt, text).await
    }

    /// Translate with a custom temperature value.
    /// Used by multi-round translation to produce varied outputs.
    pub async fn translate_with_temperature(
        &self,
        text: &str,
        from: &str,
        to: &str,
        temperature: f32,
    ) -> anyhow::Result<String> {
        let system_prompt = self.build_system_prompt(from, to, None);
        self.call_llm_with_temperature(&system_prompt, text, temperature)
            .await
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        // Use char-based truncation to avoid panicking on multi-byte UTF-8 boundaries
        let truncated: String = text.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

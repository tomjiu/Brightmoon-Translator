use super::TranslationEngine;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
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

/// Single LLM endpoint (key + URL + model + wire format) for ordered failover.
#[derive(Debug, Clone)]
pub struct LlmEndpointConfig {
    pub label: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    /// "openai" | "anthropic" | "gemini"
    pub api_format: String,
}

pub struct LlmEngine {
    endpoints: Vec<LlmEndpointConfig>,
    custom_prompt: String,
    temperature: f32,
    max_tokens: u32,
    client: Client,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Clone, Serialize)]
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
        let mut endpoints = Vec::new();
        if !api_key.is_empty() {
            endpoints.push(LlmEndpointConfig {
                label: "default".to_string(),
                api_key: api_key.to_string(),
                base_url: base_url.trim_end_matches('/').to_string(),
                model: model.to_string(),
                api_format: "openai".to_string(),
            });
        }
        Self {
            endpoints,
            custom_prompt: String::new(),
            temperature: 0.3,
            max_tokens: 4096,
            client: Client::new(),
        }
    }

    /// Multiple keys sharing one base_url/model (legacy); each key becomes an endpoint.
    pub fn with_multiple_keys(api_keys: Vec<String>, base_url: &str, model: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let model = model.to_string();
        let endpoints = api_keys
            .into_iter()
            .enumerate()
            .filter(|(_, k)| !k.is_empty())
            .map(|(i, api_key)| LlmEndpointConfig {
                label: if i == 0 {
                    "default".to_string()
                } else {
                    format!("key#{}", i + 1)
                },
                api_key,
                base_url: base_url.clone(),
                model: model.clone(),
                api_format: "openai".to_string(),
            })
            .collect();
        Self {
            endpoints,
            custom_prompt: String::new(),
            temperature: 0.3,
            max_tokens: 4096,
            client: Client::new(),
        }
    }

    /// Ordered endpoints; tried in order on HTTP/API failure (non-streaming).
    pub fn with_endpoints(endpoints: Vec<LlmEndpointConfig>) -> Self {
        let endpoints = endpoints
            .into_iter()
            .map(|mut e| {
                e.base_url = e.base_url.trim_end_matches('/').to_string();
                e
            })
            .collect();
        Self {
            endpoints,
            custom_prompt: String::new(),
            temperature: 0.3,
            max_tokens: 4096,
            client: Client::new(),
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
    fn build_request(
        &self,
        model: &str,
        system_prompt: &str,
        user_text: &str,
        stream: bool,
    ) -> ChatRequest {
        ChatRequest {
            model: model.to_string(),
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

    fn primary_model(&self) -> &str {
        self.endpoints
            .first()
            .map(|e| e.model.as_str())
            .unwrap_or("")
    }

    /// Shared non-streaming LLM call with ordered endpoint failover.
    async fn call_llm_with_messages(&self, messages: Vec<Message>) -> anyhow::Result<String> {
        self.call_llm_with_messages_temp(messages, self.temperature)
            .await
    }

    async fn call_llm_with_messages_temp(
        &self,
        messages: Vec<Message>,
        temperature: f32,
    ) -> anyhow::Result<String> {
        let total = self.endpoints.len();
        let mut last_error = String::new();

        for (attempt, ep) in self.endpoints.iter().enumerate() {
            let label = crate::security::sanitize_log_message(&ep.label);
            let format = crate::models::config::normalize_api_format(&ep.api_format);
            match self
                .call_one_endpoint(ep, &format, &messages, temperature)
                .await
            {
                Ok(content) => return Ok(content),
                Err(e) => {
                    last_error = e;
                    tracing::warn!(
                        "Endpoint '{}' attempt {} failed: {}",
                        label,
                        attempt + 1,
                        crate::security::sanitize_log_message(&last_error)
                    );
                    continue;
                },
            }
        }

        if total == 0 {
            last_error = "No LLM endpoints configured".to_string();
        }

        Err(anyhow::anyhow!(
            "All {} endpoints failed. Last error: {}",
            total,
            last_error
        ))
    }

    async fn call_one_endpoint(
        &self,
        ep: &LlmEndpointConfig,
        format: &str,
        messages: &[Message],
        temperature: f32,
    ) -> Result<String, String> {
        match format {
            "anthropic" => self.call_anthropic(ep, messages, temperature).await,
            "gemini" => self.call_gemini(ep, messages, temperature).await,
            _ => self.call_openai_compat(ep, messages, temperature).await,
        }
    }

    async fn call_openai_compat(
        &self,
        ep: &LlmEndpointConfig,
        messages: &[Message],
        temperature: f32,
    ) -> Result<String, String> {
        let request = ChatRequest {
            model: ep.model.clone(),
            messages: messages.to_vec(),
            temperature,
            max_tokens: self.max_tokens,
            stream: false,
        };
        let url = format!("{}/chat/completions", ep.base_url);
        let mut req = self.client.post(&url).json(&request);
        if !ep.api_key.is_empty() {
            req = req.bearer_auth(&ep.api_key);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(sanitize_llm_error(status, &body));
        }
        let chat_resp: ChatResponse = resp
            .json()
            .await
            .map_err(|e| format!("Parse OpenAI response: {e}"))?;
        chat_resp
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "LLM API returned no choices in response".into())
    }

    async fn call_anthropic(
        &self,
        ep: &LlmEndpointConfig,
        messages: &[Message],
        _temperature: f32,
    ) -> Result<String, String> {
        let mut system = String::new();
        let mut anth_messages = Vec::new();
        for m in messages {
            if m.role == "system" {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(&m.content);
            } else {
                let role = if m.role == "assistant" {
                    "assistant"
                } else {
                    "user"
                };
                anth_messages.push(serde_json::json!({
                    "role": role,
                    "content": m.content,
                }));
            }
        }
        if anth_messages.is_empty() {
            return Err("Anthropic: no user messages".into());
        }

        let mut body = serde_json::json!({
            "model": ep.model,
            "max_tokens": self.max_tokens,
            "messages": anth_messages,
        });
        if !system.is_empty() {
            body["system"] = serde_json::json!(system);
        }

        let url = format!("{}/messages", ep.base_url.trim_end_matches('/'));
        let mut req = self
            .client
            .post(&url)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body);
        if !ep.api_key.is_empty() {
            req = req.header("x-api-key", &ep.api_key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Anthropic request failed: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(sanitize_llm_error(status, &text));
        }
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Anthropic JSON: {e}"))?;
        let mut out = String::new();
        if let Some(arr) = v.get("content").and_then(|c| c.as_array()) {
            for part in arr {
                if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                        out.push_str(t);
                    }
                }
            }
        }
        let out = out.trim().to_string();
        if out.is_empty() {
            Err("Anthropic: empty content".into())
        } else {
            Ok(out)
        }
    }

    async fn call_gemini(
        &self,
        ep: &LlmEndpointConfig,
        messages: &[Message],
        temperature: f32,
    ) -> Result<String, String> {
        let mut system_bits = Vec::new();
        let mut contents = Vec::new();
        for m in messages {
            if m.role == "system" {
                system_bits.push(m.content.clone());
                continue;
            }
            let role = if m.role == "assistant" {
                "model"
            } else {
                "user"
            };
            contents.push(serde_json::json!({
                "role": role,
                "parts": [{ "text": m.content }]
            }));
        }
        if contents.is_empty() {
            return Err("Gemini: no user messages".into());
        }

        let mut body = serde_json::json!({
            "contents": contents,
            "generationConfig": {
                "temperature": temperature,
                "maxOutputTokens": self.max_tokens,
            }
        });
        if !system_bits.is_empty() {
            body["systemInstruction"] = serde_json::json!({
                "parts": [{ "text": system_bits.join("\n") }]
            });
        }

        let base = ep.base_url.trim_end_matches('/');
        let model = ep.model.trim().trim_start_matches("models/");
        let mut url = format!("{base}/models/{model}:generateContent");
        if !ep.api_key.is_empty() {
            url.push_str(&format!("?key={}", urlencoding::encode(&ep.api_key)));
        }

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Gemini request failed: {e}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(sanitize_llm_error(status, &text));
        }
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Gemini JSON: {e}"))?;
        let mut out = String::new();
        if let Some(cands) = v.get("candidates").and_then(|c| c.as_array()) {
            if let Some(parts) = cands
                .first()
                .and_then(|c| c.pointer("/content/parts"))
                .and_then(|p| p.as_array())
            {
                for p in parts {
                    if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                        out.push_str(t);
                    }
                }
            }
        }
        let out = out.trim().to_string();
        if out.is_empty() {
            Err("Gemini: empty candidates".into())
        } else {
            Ok(out)
        }
    }

    /// Non-streaming LLM call with system_prompt and user_text convenience wrapper
    async fn call_llm(&self, system_prompt: &str, user_text: &str) -> anyhow::Result<String> {
        let messages = self
            .build_request(self.primary_model(), system_prompt, user_text, false)
            .messages;
        self.call_llm_with_messages(messages).await
    }

    /// Non-streaming LLM call with a custom temperature value (endpoint failover).
    async fn call_llm_with_temperature(
        &self,
        system_prompt: &str,
        user_text: &str,
        temperature: f32,
    ) -> anyhow::Result<String> {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: user_text.to_string(),
            },
        ];
        self.call_llm_with_messages_temp(messages, temperature)
            .await
    }

    /// Streaming uses the first endpoint only; non-openai formats fall back to non-stream.
    async fn stream_llm(
        &self,
        messages: Vec<Message>,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        let total = self.endpoints.len();
        if total == 0 {
            return Err(anyhow::anyhow!("No LLM endpoints configured"));
        }

        let mut last_error = String::new();

        for (attempt, ep) in self.endpoints.iter().enumerate() {
            let label = crate::security::sanitize_log_message(&ep.label);
            // P0 fix: each endpoint streams into a PRIVATE channel first. If the
            // endpoint fails partway (HTTP error / mid-stream), its partial tokens
            // are DISCARDED — never forwarded to the caller's channel. Only a
            // fully-successful endpoint's tokens are relayed to `tx`. This prevents
            // "Hello, " (endpoint 1) + "World" (endpoint 2) concatenation in the
            // consumer when failover kicks in mid-stream.
            let (inner_tx, mut inner_rx) = mpsc::channel::<String>(100);
            match self.stream_one_endpoint(ep, &messages, &inner_tx).await {
                Ok(text) => {
                    drop(inner_tx);
                    while let Some(token) = inner_rx.recv().await {
                        let _ = tx.send(token).await;
                    }
                    return Ok(text);
                },
                Err(e) => {
                    last_error = format!("{}", e);
                    tracing::warn!(
                        "Stream endpoint '{}' attempt {} of {} failed: {}",
                        label,
                        attempt + 1,
                        total,
                        crate::security::sanitize_log_message(&last_error)
                    );
                    continue;
                },
            }
        }

        Err(anyhow::anyhow!(
            "All {} stream endpoints failed. Last error: {}",
            total,
            last_error
        ))
    }

    async fn stream_one_endpoint(
        &self,
        ep: &LlmEndpointConfig,
        messages: &[Message],
        tx: &mpsc::Sender<String>,
    ) -> anyhow::Result<String> {

        let format = crate::models::config::normalize_api_format(&ep.api_format);
        if format != "openai" {
            let content = self
                .call_one_endpoint(ep, &format, &messages, self.temperature)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            let _ = tx.send(content.clone()).await;
            return Ok(content);
        }

        let request = ChatRequest {
            model: ep.model.clone(),
            messages: messages.to_vec(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: true,
        };

        let url = format!("{}/chat/completions", ep.base_url);
        let mut req = self.client.post(&url).json(&request);
        if !ep.api_key.is_empty() {
            req = req.bearer_auth(&ep.api_key);
        }

        let resp = req.send().await.map_err(|e| {
            anyhow::anyhow!(
                "Stream request failed on endpoint '{}': {}",
                crate::security::sanitize_log_message(&ep.label),
                e
            )
        })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(sanitize_llm_error(status, &body)));
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

        Ok(full_text)
    }

    pub async fn translate_stream(
        &self,
        text: &str,
        from: &str,
        to: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        let system_prompt = self.build_system_prompt(from, to, None);
        let messages = self
            .build_request(self.primary_model(), &system_prompt, text, true)
            .messages;
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
        let messages = self
            .build_request(self.primary_model(), &system_prompt, text, true)
            .messages;
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

    /// Multi-segment batch: send numbered list, parse numbered reply (fallback: line split).
    pub async fn translate_batch_segments(
        &self,
        segments: &[&str],
        from: &str,
        to: &str,
    ) -> anyhow::Result<Vec<String>> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }
        if segments.len() == 1 {
            let one = self.translate(segments[0], from, to).await?;
            return Ok(vec![one]);
        }

        let numbered_input = format_numbered_batch_input(segments);
        let base = self.build_system_prompt(from, to, None);
        let system_prompt = format!(
            "{base}\n\n输入为编号列表（1. 2. …）。请按相同编号逐条翻译，仅返回编号译文，不要解释。"
        );
        let raw = self.call_llm(&system_prompt, &numbered_input).await?;
        Ok(split_batch_response(&raw, segments.len()))
    }
}

/// Pack segments as `1. …\n2. …` for multi-seg LLM batch.
pub fn format_numbered_batch_input(segments: &[&str]) -> String {
    segments
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}. {}", i + 1, s))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Prefer numbered parse; on failure split non-empty lines and pad/truncate to `expected`.
pub fn split_batch_response(response: &str, expected: usize) -> Vec<String> {
    if expected == 0 {
        return Vec::new();
    }
    if let Some(parsed) = crate::response_check::parse_numbered_response(response, expected) {
        return parsed;
    }
    // Fallback: line split (previous multi-seg behaviour)
    let mut lines: Vec<String> = response
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    while lines.len() < expected {
        lines.push(String::new());
    }
    lines.truncate(expected);
    lines
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_numbered_batch_input() {
        let segs = ["a", "b"];
        assert_eq!(format_numbered_batch_input(&segs), "1. a\n2. b");
    }

    #[test]
    fn test_split_batch_response_numbered() {
        let raw = "1. 你好\n2. 世界";
        assert_eq!(
            split_batch_response(raw, 2),
            vec!["你好".to_string(), "世界".to_string()]
        );
    }

    #[test]
    fn test_split_batch_response_fallback_lines() {
        let raw = "hello\nworld\nextra";
        assert_eq!(
            split_batch_response(raw, 2),
            vec!["hello".to_string(), "world".to_string()]
        );
    }

    #[test]
    fn test_split_batch_response_fallback_pad() {
        let raw = "only-one";
        assert_eq!(
            split_batch_response(raw, 2),
            vec!["only-one".to_string(), String::new()]
        );
    }
}

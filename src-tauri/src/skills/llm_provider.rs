// LLM Provider - 统一的 LLM 抽象层

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// LLM 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

/// LLM 请求
#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub messages: Vec<LlmMessage>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub json_schema: Option<Value>,
}

impl LlmRequest {
    pub fn new(messages: Vec<LlmMessage>) -> Self {
        Self {
            messages,
            temperature: 0.7,
            max_tokens: 2000,
            json_schema: None,
        }
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_json_schema(mut self, schema: Value) -> Self {
        self.json_schema = Some(schema);
        self
    }
}

/// LLM 响应
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    pub usage: LlmUsage,
}

/// LLM 使用统计
#[derive(Debug, Clone, Default)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// LLM Provider Trait
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider 名称
    fn name(&self) -> &str;

    /// 完成请求（非流式）
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse>;

    /// 流式完成（可选）
    async fn complete_stream(
        &self,
        _request: LlmRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        anyhow::bail!("Streaming not supported by this provider")
    }

    /// 是否可用
    fn is_available(&self) -> bool {
        true
    }
}

/// OpenAI 兼容的 Provider（适用于 GPT-4, `DeepSeek` 等）
pub struct OpenAiCompatibleProvider {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2 * 60))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            api_key,
            base_url,
            model,
            client,
        }
    }

    /// OpenAI 官方
    pub fn openai(api_key: String, model: String) -> Self {
        Self::new(api_key, "https://api.openai.com/v1".to_string(), model)
    }

    /// 当前使用的模型名
    pub fn model_name(&self) -> String {
        self.model.clone()
    }

    /// API Key（learning_plan_cmd 批量生成复用）
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// `DeepSeek`
    pub fn deepseek(api_key: String) -> Self {
        Self::new(
            api_key,
            "https://api.deepseek.com".to_string(),
            "deepseek-chat".to_string(),
        )
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &'static str {
        "openai_compatible"
    }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        #[derive(Serialize)]
        struct ApiRequest {
            model: String,
            messages: Vec<LlmMessage>,
            temperature: f32,
            max_tokens: u32,
            #[serde(skip_serializing_if = "Option::is_none")]
            response_format: Option<serde_json::Value>,
        }

        #[derive(Deserialize)]
        struct ApiResponse {
            choices: Vec<ApiChoice>,
            usage: Option<ApiUsage>,
            model: String,
        }

        #[derive(Deserialize)]
        struct ApiChoice {
            message: LlmMessage,
        }

        #[derive(Deserialize)]
        #[allow(clippy::struct_field_names)]
        struct ApiUsage {
            prompt_tokens: u32,
            completion_tokens: u32,
            total_tokens: u32,
        }

        // 将 json_schema 要求嵌入 system prompt（兼容所有 API 后端）
        let mut messages = request.messages;
        if let Some(schema) = &request.json_schema {
            let schema_str = serde_json::to_string_pretty(schema).unwrap_or_default();
            let instruction = format!(
                "\n\n请严格按照以下 JSON Schema 返回结果，不要包含任何其他文字，只返回合法的 JSON：\n```json\n{schema_str}\n```"
            );
            if let Some(last) = messages.last_mut() {
                if last.role == "user" {
                    last.content.push_str(&instruction);
                } else {
                    messages.push(LlmMessage::user(&instruction));
                }
            }
        }

        let api_request = ApiRequest {
            model: self.model.clone(),
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            response_format: request.json_schema.map(|_| {
                serde_json::json!({ "type": "json_object" })
            }),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&api_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            anyhow::bail!("LLM API error {status}: {body}");
        }

        let api_response: ApiResponse = response.json().await?;

        let usage = api_response.usage.unwrap_or(ApiUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        });

        Ok(LlmResponse {
            content: api_response.choices[0].message.content.clone(),
            model: api_response.model,
            usage: LlmUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            },
        })
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}

/// 从 LLM 配置构建可用的 OpenAI 兼容 Provider。
/// 优先使用 `api_key，其次` `api_keys` `列表第一个；api_key` 为空或 `base_url` 为空时返回 None。
pub fn provider_from_config(
    llm: &crate::models::config::LlmConfig,
) -> Option<OpenAiCompatibleProvider> {
    if llm.base_url.is_empty() {
        return None;
    }
    let api_key = if llm.api_key.is_empty() {
        llm.api_keys.first().cloned()
    } else {
        Some(llm.api_key.clone())
    };
    let key = api_key?;
    Some(OpenAiCompatibleProvider::new(
        key,
        llm.base_url.clone(),
        llm.model.clone(),
    ))
}

/// 按 provider id 从多提供商列表解析 LLM Provider。
/// `id` 匹配 `llm.providers` 中已启用的条目，取其 api_key/base_url/model，
/// 单条缺失时回退到 LlmConfig 顶层字段。找不到或未启用时返回 None。
pub fn provider_from_config_by_id(
    llm: &crate::models::config::LlmConfig,
    id: &str,
) -> Option<OpenAiCompatibleProvider> {
    let entry = llm
        .providers
        .iter()
        .find(|p| p.id == id && p.enabled && !p.api_key.trim().is_empty())
        .or_else(|| llm.providers.iter().find(|p| p.id == id))?;

    let api_key = entry.api_key.trim().to_string();
    if api_key.is_empty() {
        return None;
    }
    let base_url = if entry.base_url.trim().is_empty() {
        llm.base_url.trim().to_string()
    } else {
        entry.base_url.trim().to_string()
    };
    let model = if entry.model.trim().is_empty() {
        llm.model.trim().to_string()
    } else {
        entry.model.trim().to_string()
    };
    if base_url.is_empty() {
        return None;
    }
    Some(OpenAiCompatibleProvider::new(api_key, base_url, model))
}

/// 解析 AI 学习系统专用的 LLM Provider：
/// 优先用 `learn_llm_provider_id`（多提供商列表），未配置时跟随全局。
pub fn provider_from_config_for_learning(
    config: &crate::models::config::AppConfig,
) -> Option<OpenAiCompatibleProvider> {
    match config.learn_llm_provider_id.as_deref() {
        Some(id) if !id.is_empty() => {
            provider_from_config_by_id(&config.llm, id)
                .or_else(|| provider_from_config(&config.llm))
        },
        _ => provider_from_config(&config.llm),
    }
}

/// 从 LLM 响应中提取 JSON 字符串。
/// 处理场景：直接以 `{`/`[` 开头、markdown 代码块包裹（```json ... ```）、
/// 以及前后夹杂文本（提取首尾 `{...}` 或 `[...]`）。
pub fn extract_json(content: &str) -> &str {
    let trimmed = content.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return trimmed;
    }
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }
    // 提取第一个 `{...}` 或 `[...]` 完整块
    for (open, close) in [('{', '}'), ('[', ']')] {
        if let Some(start) = trimmed.find(open) {
            if let Some(end) = trimmed[start..].rfind(close) {
                return &trimmed[start..=(start + end)];
            }
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        assert_eq!(extract_json("{a:1}"), "{a:1}");
        assert_eq!(extract_json("[1,2]"), "[1,2]");
    }

    #[test]
    fn test_extract_json_markdown_block() {
        assert_eq!(extract_json("```json\n{\"b\":2}\n```"), "{\"b\":2}");
    }

    #[test]
    fn test_extract_json_embedded_text() {
        assert_eq!(extract_json("前缀 {\"c\":3} 后缀"), "{\"c\":3}");
        assert_eq!(extract_json("答案: [1, 2, 3] 完"), "[1, 2, 3]");
    }

    #[test]
    fn test_extract_json_no_json_returns_trimmed() {
        assert_eq!(extract_json("  无 JSON 内容  "), "无 JSON 内容");
    }

    #[test]
    fn test_llm_message() {
        let msg = LlmMessage::user("Hello");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_llm_request_builder() {
        let request = LlmRequest::new(vec![LlmMessage::user("test")])
            .with_temperature(0.5)
            .with_max_tokens(1000);

        assert_eq!(request.temperature, 0.5);
        assert_eq!(request.max_tokens, 1000);
    }

    #[tokio::test]
    #[ignore = "需要真实 API key"]
    async fn test_openai_provider() {
        let provider = OpenAiCompatibleProvider::openai(
            std::env::var("OPENAI_API_KEY").unwrap(),
            "gpt-4".to_string(),
        );

        let request = LlmRequest::new(vec![
            LlmMessage::system("You are a helpful assistant."),
            LlmMessage::user("Say hello!"),
        ]);

        let response = provider.complete(request).await.unwrap();
        assert!(!response.content.is_empty());
    }
}

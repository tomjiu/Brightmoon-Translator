// Dictionary Source System - T7 可插拔词典源
//
// 定义统一的 DictionarySource trait,让 ECDICT / 有道 / 在线API / AI Prompt
// 都作为可插拔的词典源,聚合到统一的词典查询流程。

use crate::skills::{LlmMessage, LlmProvider, LlmRequest, OpenAiCompatibleProvider};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 统一的词典条目结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictEntryResult {
    pub word: String,
    pub phonetics: Vec<String>,
    pub chinese_translation: Option<String>,
    pub english_definitions: Vec<String>,
    pub pos: Vec<String>,
    pub examples: Vec<String>,
    pub source: String,
    pub raw: Option<String>,
}

/// 词典源配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictSourceConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    /// AI Prompt 源专用：用户自定义 prompt 模板
    pub prompt_template: Option<String>,
}

/// 词典源 trait
#[async_trait]
pub trait DictionarySource: Send + Sync {
    /// 唯一 ID
    fn id(&self) -> &str;

    /// 显示名称
    fn name(&self) -> &str;

    /// 查询单词
    async fn lookup(&self, word: &str) -> Result<DictEntryResult>;

    /// 是否可用（在线源可能因配置不可用）
    fn is_available(&self) -> bool {
        true
    }
}

// ============================================
// ECDICT Source
// ============================================

/// ECDICT 本地词典源
pub struct EcdictSource {
    pool: sqlx::SqlitePool,
}

impl EcdictSource {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DictionarySource for EcdictSource {
    fn id(&self) -> &str {
        "ecdict"
    }

    fn name(&self) -> &str {
        "ECDICT"
    }

    async fn lookup(&self, word: &str) -> Result<DictEntryResult> {
        use sqlx::Row;

        let row = sqlx::query(
            "SELECT word, phonetic, definition, translation, pos, exchange FROM stardict WHERE word = ?1",
        )
        .bind(word)
        .fetch_optional(&self.pool)
        .await
        .context("ECDICT 查询失败")?;

        let Some(row) = row else {
            bail!("ECDICT: 未找到 '{}'", word);
        };

        let definition: Option<String> = row.try_get("definition").unwrap_or_default();
        let translation: Option<String> = row.try_get("translation").unwrap_or_default();
        let phonetic: Option<String> = row.try_get("phonetic").unwrap_or_default();
        let pos: Option<String> = row.try_get("pos").unwrap_or_default();
        let exchange: Option<String> = row.try_get("exchange").unwrap_or_default();

        let english_definitions = definition
            .as_deref()
            .map(|d| {
                d.split('\n')
                    .filter(|s| !s.trim().is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let phonetics = phonetic
            .map(|p| {
                vec![p.trim().trim_start_matches('/').trim_end_matches('/').to_string()]
            })
            .unwrap_or_default();

        let pos_list = pos
            .as_deref()
            .map(|p| {
                p.split('/')
                    .filter(|s| !s.trim().is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        Ok(DictEntryResult {
            word: row.try_get("word").unwrap_or_else(|_| word.to_string()),
            phonetics,
            chinese_translation: translation,
            english_definitions,
            pos: pos_list,
            examples: Vec::new(),
            source: "ECDICT".to_string(),
            raw: exchange,
        })
    }
}

// ============================================
// Online DictionaryAPI Source
// ============================================

/// DictionaryAPI.dev 在线词典源
pub struct OnlineApiSource {
    client: reqwest::Client,
}

impl OnlineApiSource {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl Default for OnlineApiSource {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct OnlineApiWord {
    word: String,
    #[allow(dead_code)]
    phonetics: Vec<serde_json::Value>,
    meanings: Vec<OnlineApiMeaning>,
}

#[derive(Debug, Deserialize)]
struct OnlineApiMeaning {
    #[serde(rename = "partOfSpeech")]
    part_of_speech: String,
    definitions: Vec<OnlineApiDefinition>,
}

#[derive(Debug, Deserialize)]
struct OnlineApiDefinition {
    definition: String,
    example: Option<String>,
}

#[async_trait]
impl DictionarySource for OnlineApiSource {
    fn id(&self) -> &str {
        "online_api"
    }

    fn name(&self) -> &str {
        "DictionaryAPI.dev"
    }

    async fn lookup(&self, word: &str) -> Result<DictEntryResult> {
        let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", word);
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            bail!("在线词典请求失败: {}", response.status());
        }

        let data: Vec<OnlineApiWord> = response.json().await?;
        let Some(first) = data.first() else {
            bail!("在线词典无结果");
        };

        let mut english_definitions = Vec::new();
        let mut pos_list = Vec::new();
        let mut examples = Vec::new();

        for m in &first.meanings {
            pos_list.push(m.part_of_speech.clone());
            for d in &m.definitions {
                english_definitions.push(d.definition.clone());
                if let Some(ex) = &d.example {
                    examples.push(ex.clone());
                }
            }
        }

        Ok(DictEntryResult {
            word: first.word.clone(),
            phonetics: Vec::new(),
            chinese_translation: None,
            english_definitions,
            pos: pos_list,
            examples,
            source: "DictionaryAPI.dev".to_string(),
            raw: None,
        })
    }
}

// ============================================
// AI Prompt Source
// ============================================

/// AI Prompt 词典源
///
/// 用 LLM 按用户自定义模板生成词典条目。模板可自定义输出结构,
/// 结果带 JSON Schema 强制 + 解析失败降温度重试。
pub struct AiPromptSource {
    api_key: String,
    base_url: String,
    model: String,
    /// 用户自定义 prompt 模板（默认英文词典条目）
    template: Option<String>,
}

impl AiPromptSource {
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        Self {
            api_key,
            base_url,
            model,
            template: None,
        }
    }

    pub fn with_template(mut self, template: String) -> Self {
        self.template = Some(template);
        self
    }
}

/// AI 词典条目的 JSON 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiDictPayload {
    phonetic: Option<String>,
    chinese_translation: String,
    english_definitions: Vec<String>,
    parts_of_speech: Vec<String>,
    examples: Vec<String>,
}

const DEFAULT_TEMPLATE: &str = r#"
你是词典。为单词 "{word}" 生成学习词典条目：
1. 音标(phonetic)
2. 中文释义(chinese_translation)
3. 英文释义(english_definitions, 2-4条)
4. 词性(parts_of_speech)
5. 例句(examples, 2个, 中英对照)
只返回 JSON，不要其他文字。
"#;

#[async_trait]
impl DictionarySource for AiPromptSource {
    fn id(&self) -> &str {
        "ai_prompt"
    }

    fn name(&self) -> &str {
        "AI Prompt"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty() && !self.base_url.is_empty()
    }

    async fn lookup(&self, word: &str) -> Result<DictEntryResult> {
        if !self.is_available() {
            bail!("AI 词典源未配置 LLM");
        }

        let template = self
            .template
            .clone()
            .unwrap_or_else(|| DEFAULT_TEMPLATE.to_string());
        let user_prompt = template.replace("{word}", word);

        let provider = OpenAiCompatibleProvider::new(
            self.api_key.clone(),
            self.base_url.clone(),
            self.model.clone(),
        );

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "phonetic": { "type": "string" },
                "chinese_translation": { "type": "string" },
                "english_definitions": { "type": "array", "items": { "type": "string" } },
                "parts_of_speech": { "type": "array", "items": { "type": "string" } },
                "examples": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["chinese_translation", "english_definitions", "parts_of_speech", "examples"]
        });

        // 失败自动重试一次（降温度）
        let mut payload: Option<AiDictPayload> = None;
        let mut last_err: Option<anyhow::Error> = None;

        for (attempt, temperature) in [(0, 0.5f32), (1, 0.2f32)].iter() {
            let request = LlmRequest::new(vec![
                LlmMessage::system("你是严格的词典工具，只返回合法 JSON。"),
                LlmMessage::user(user_prompt.clone()),
            ])
            .with_temperature(*temperature)
            .with_max_tokens(1500)
            .with_json_schema(schema.clone());

            match provider.complete(request).await {
                Ok(r) => {
                    let json_str = extract_json(&r.content);
                    match serde_json::from_str::<AiDictPayload>(json_str) {
                        Ok(p) => {
                            payload = Some(p);
                            break;
                        },
                        Err(e) => {
                            tracing::warn!(
                                "AI 词典 JSON 解析失败 attempt={}: {}",
                                attempt,
                                e
                            );
                            last_err = Some(anyhow::anyhow!(
                                "AI 词典返回格式错误: {}",
                                e
                            ));
                        },
                    }
                },
                Err(e) => {
                    tracing::warn!("AI 词典生成失败 attempt={}: {}", attempt, e);
                    last_err = Some(e);
                },
            }
        }

        let p = payload.ok_or_else(|| last_err.unwrap_or_else(|| anyhow::anyhow!("AI 词典失败")))?;

        Ok(DictEntryResult {
            word: word.to_string(),
            phonetics: p.phonetic.map(|ph| vec![ph]).unwrap_or_default(),
            chinese_translation: Some(p.chinese_translation),
            english_definitions: p.english_definitions,
            pos: p.parts_of_speech,
            examples: p.examples,
            source: "AI Prompt".to_string(),
            raw: None,
        })
    }
}

/// 从 LLM 响应中提取 JSON
fn extract_json(content: &str) -> &str {
    let trimmed = content.trim();
    if trimmed.starts_with('{') {
        return trimmed;
    }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed[start..].rfind('}') {
            return &trimmed[start..start + end + 1];
        }
    }
    trimmed
}

// ============================================
// Source Registry
// ============================================

/// 词典源注册表（管理启用的源 + 优先级）
pub struct SourceRegistry {
    sources: Vec<Box<dyn DictionarySource>>,
    /// 已启用的 source id 列表
    enabled: Vec<String>,
    /// source id -> 自定义模板
    templates: std::collections::HashMap<String, String>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            enabled: Vec::new(),
            templates: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, source: Box<dyn DictionarySource>) {
        let id = source.id().to_string();
        self.sources.push(source);
        self.enabled.push(id);
    }

    /// 设置启用的源（按配置覆盖）
    pub fn set_enabled(&mut self, ids: Vec<String>) {
        self.enabled = ids;
    }

    /// 设置某个源的 prompt 模板
    pub fn set_template(&mut self, source_id: &str, template: String) {
        self.templates.insert(source_id.to_string(), template);
    }

    /// 查询所有启用的源（聚合结果，按注册顺序）
    pub async fn lookup_all(&self, word: &str) -> Vec<DictEntryResult> {
        // 并行查询所有启用源，避免串行等待（尤其 AI 源慢时）
        let futures: Vec<_> = self
            .sources
            .iter()
            .filter(|s| self.enabled.contains(&s.id().to_string()))
            .map(|s| async {
                match s.lookup(word).await {
                    Ok(entry) => Some(entry),
                    Err(e) => {
                        tracing::debug!("词典源 '{}' 查询 '{}' 失败: {}", s.id(), word, e);
                        None
                    },
                }
            })
            .collect();
        let results = futures::future::join_all(futures).await;
        results.into_iter().flatten().collect()
    }

    /// 按优先级取第一个成功结果
    pub async fn lookup_first(&self, word: &str) -> Option<DictEntryResult> {
        for source in &self.sources {
            let id = source.id().to_string();
            if !self.enabled.contains(&id) {
                continue;
            }
            if let Ok(entry) = source.lookup(word).await {
                return Some(entry);
            }
        }
        None
    }

    /// 源列表
    pub fn list(&self) -> Vec<DictSourceConfig> {
        self.sources
            .iter()
            .map(|s| {
                let id = s.id().to_string();
                let template = self.templates.get(&id).cloned();
                DictSourceConfig {
                    id,
                    name: s.name().to_string(),
                    enabled: self.enabled.contains(&s.id().to_string()),
                    priority: self.sources.iter().position(|x| x.id() == s.id()).unwrap_or(0) as i32,
                    prompt_template: template,
                }
            })
            .collect()
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json() {
        assert_eq!(extract_json("{a:1}"), "{a:1}");
        assert_eq!(extract_json("```json\n{\"b\":2}\n```"), "{\"b\":2}");
        assert_eq!(extract_json("前缀 {\"c\":3} 后缀"), "{\"c\":3}");
    }
}

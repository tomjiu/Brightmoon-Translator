// Generate Card Skill - AI 生成卡牌内容

use super::llm_provider::{LlmMessage, LlmProvider, LlmRequest};
use super::{Skill, SkillInput, SkillOutput};
use crate::domain::{AiContent, Etymology, Mnemonic, MnemonicType, PersonalizedExample, Root};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 生成卡牌的输入上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardContext {
    pub word: String,
    pub definition: Option<String>,
    pub translation: Option<String>,
    pub morphology: Option<String>,
}

/// AI 生成的原始输出（匹配 JSON Schema）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiGeneratedContent {
    etymology: Option<EtymologySchema>,
    mnemonics: Vec<MnemonicSchema>,
    examples: Vec<ExampleSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EtymologySchema {
    origin: String,
    roots: Vec<RootSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RootSchema {
    part: String,
    meaning: String,
    examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MnemonicSchema {
    #[serde(rename = "type")]
    mnemonic_type: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExampleSchema {
    text: String,
    context: String,
    difficulty: String,
}

/// Generate Card Skill
pub struct GenerateCardSkill {
    provider: Arc<dyn LlmProvider>,
}

impl GenerateCardSkill {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// 构建系统提示
    fn build_system_prompt(&self) -> String {
        r#"你是一个专业的英语学习内容生成助手。你的任务是为英语单词生成高质量的学习内容。

要求：
1. 词源分析要准确，基于真实的语言学知识
2. 助记法要实用、有创意，帮助学习者记忆
3. 例句要地道、实用，符合现代英语用法
4. 内容要简洁、清晰，避免过于学术化

输出格式：严格按照 JSON Schema 输出。"#
            .to_string()
    }

    /// 构建用户提示
    fn build_user_prompt(&self, context: &CardContext) -> String {
        let mut prompt = format!("请为单词 '{}' 生成学习内容。\n\n", context.word);

        if let Some(def) = &context.definition {
            prompt.push_str(&format!("定义: {}\n", def));
        }

        if let Some(trans) = &context.translation {
            prompt.push_str(&format!("翻译: {}\n", trans));
        }

        if let Some(morph) = &context.morphology {
            prompt.push_str(&format!("词根拆解: {}\n", morph));
        }

        prompt.push_str("\n请生成：\n");
        prompt.push_str("1. 词源信息（如果有）\n");
        prompt.push_str("2. 2-3个助记法（不同类型）\n");
        prompt.push_str("3. 3个例句（不同难度和场景）\n");

        prompt
    }

    /// 构建 JSON Schema
    fn build_json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "etymology": {
                    "type": "object",
                    "properties": {
                        "origin": { "type": "string" },
                        "roots": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "part": { "type": "string" },
                                    "meaning": { "type": "string" },
                                    "examples": {
                                        "type": "array",
                                        "items": { "type": "string" }
                                    }
                                },
                                "required": ["part", "meaning", "examples"]
                            }
                        }
                    },
                    "required": ["origin", "roots"]
                },
                "mnemonics": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": ["etymology", "scene", "homophone", "visual", "chunking"]
                            },
                            "content": { "type": "string" }
                        },
                        "required": ["type", "content"]
                    }
                },
                "examples": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string" },
                            "context": { "type": "string" },
                            "difficulty": { "type": "string" }
                        },
                        "required": ["text", "context", "difficulty"]
                    }
                }
            },
            "required": ["mnemonics", "examples"]
        })
    }

    /// 转换 AI 输出为 AiContent
    fn convert_to_ai_content(&self, generated: AiGeneratedContent) -> AiContent {
        let etymology = generated.etymology.map(|e| Etymology {
            origin: e.origin,
            root_breakdown: e
                .roots
                .into_iter()
                .map(|r| Root {
                    part: r.part,
                    meaning: r.meaning,
                    examples: r.examples,
                })
                .collect(),
            historical_usage: None,
            cognates: vec![],
        });

        let mnemonics = generated
            .mnemonics
            .into_iter()
            .map(|m| {
                let mnemonic_type = match m.mnemonic_type.as_str() {
                    "etymology" => MnemonicType::Etymology,
                    "scene" => MnemonicType::Scene,
                    "homophone" => MnemonicType::Homophone,
                    "visual" => MnemonicType::Visual,
                    "chunking" => MnemonicType::Chunking,
                    _ => MnemonicType::Etymology,
                };

                Mnemonic {
                    mnemonic_type,
                    content: m.content,
                    score: None,
                }
            })
            .collect();

        let examples = generated
            .examples
            .into_iter()
            .map(|e| PersonalizedExample {
                text: e.text,
                context: e.context,
                difficulty: e.difficulty,
                score: None,
                user_feedback: None,
            })
            .collect();

        AiContent {
            etymology,
            mnemonics,
            examples,
            scenes: vec![],
        }
    }
}

#[async_trait]
impl Skill for GenerateCardSkill {
    fn name(&self) -> &str {
        "generate_card"
    }

    fn description(&self) -> &str {
        "使用 AI 生成单词卡牌的学习内容（词源、助记法、例句）"
    }

    async fn execute(&self, input: SkillInput) -> Result<SkillOutput> {
        // 解析上下文
        let context: CardContext = if let Some(ctx) = input.get_param("context") {
            serde_json::from_value(ctx.clone())?
        } else {
            CardContext {
                word: input.primary.clone(),
                definition: None,
                translation: None,
                morphology: None,
            }
        };

        // 构建 LLM 请求
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(&context);
        let json_schema = self.build_json_schema();

        let request = LlmRequest::new(vec![
            LlmMessage::system(system_prompt),
            LlmMessage::user(user_prompt),
        ])
        .with_temperature(0.7)
        .with_max_tokens(2000)
        .with_json_schema(json_schema);

        // 调用 LLM
        let response = self.provider.complete(request).await?;

        // 解析响应
        let generated: AiGeneratedContent = serde_json::from_str(&response.content)?;

        // 转换为 AiContent
        let ai_content = self.convert_to_ai_content(generated);

        // 返回结果
        Ok(SkillOutput::from_json(&ai_content)?
            .with_metadata("model", serde_json::json!(response.model))
            .with_metadata("tokens", serde_json::json!(response.usage.total_tokens))
            .with_metadata("confidence", serde_json::json!(0.9)))
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        if input.primary.is_empty() {
            anyhow::bail!("单词不能为空");
        }
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.provider.is_available()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompts() {
        use super::super::llm_provider::OpenAiCompatibleProvider;

        let provider = Arc::new(OpenAiCompatibleProvider::openai(
            "test".to_string(),
            "gpt-4".to_string(),
        ));
        let skill = GenerateCardSkill::new(provider);

        let context = CardContext {
            word: "brilliant".to_string(),
            definition: Some("very bright".to_string()),
            translation: Some("出色的".to_string()),
            morphology: Some("brill.i.ant".to_string()),
        };

        let system = skill.build_system_prompt();
        assert!(system.contains("学习内容"));

        let user = skill.build_user_prompt(&context);
        assert!(user.contains("brilliant"));
        assert!(user.contains("very bright"));
    }
}

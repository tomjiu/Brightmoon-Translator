// Optimize Card Skill - AI 优化卡牌内容

use super::llm_provider::{LlmMessage, LlmProvider, LlmRequest};
use super::{Skill, SkillInput, SkillOutput};
use crate::domain::{CardPatch, OptimizeTrigger, PatchOperation, WordCard};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 优化上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeContext {
    pub word: String,
    pub current_content: String,
    pub triggers: Vec<OptimizeTrigger>,
    pub error_history: Option<String>,
}

/// AI 生成的优化建议
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OptimizationResult {
    patches: Vec<PatchSchema>,
    reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PatchSchema {
    target_field: String,
    operation: String,
    new_value: serde_json::Value,
    reasoning: String,
    confidence: f64,
}

/// Optimize Card Skill
pub struct OptimizeCardSkill {
    provider: Arc<dyn LlmProvider>,
}

impl OptimizeCardSkill {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// 构建系统提示
    fn build_system_prompt(&self) -> String {
        r#"你是一个专业的学习内容优化助手。你的任务是分析用户的学习困难，提出针对性的优化建议。

原则：
1. 根据触发原因（低分、遗忘、错误）进行分析
2. 提出具体的改进方案（不要泛泛而谈）
3. 保持内容简洁、实用
4. 给出合理的置信度评分

输出格式：严格按照 JSON Schema 输出。"#
            .to_string()
    }

    /// 构建用户提示
    fn build_user_prompt(&self, context: &OptimizeContext) -> String {
        let mut prompt = format!("单词: {}\n\n", context.word);

        prompt.push_str("当前学习内容:\n");
        prompt.push_str(&context.current_content);
        prompt.push_str("\n\n");

        prompt.push_str("遇到的问题:\n");
        for trigger in &context.triggers {
            let desc = match trigger {
                OptimizeTrigger::LowRating { score } => {
                    format!("- 用户打分过低 ({}分)", score)
                },
                OptimizeTrigger::FrequentLapses { lapses } => {
                    format!("- 频繁遗忘 ({}次)", lapses)
                },
                OptimizeTrigger::UserFeedback { feedback } => {
                    format!("- 用户反馈: {}", feedback)
                },
                OptimizeTrigger::ErrorDetected { error_type } => {
                    format!("- 检测到错误: {}", error_type)
                },
            };
            prompt.push_str(&desc);
            prompt.push_str("\n");
        }

        if let Some(history) = &context.error_history {
            prompt.push_str(&format!("\n错误历史:\n{}\n", history));
        }

        prompt.push_str("\n请分析问题原因，并提出优化建议（patches）。\n");
        prompt.push_str("可以优化的字段: mnemonic, examples, etymology\n");

        prompt
    }

    /// 构建 JSON Schema
    fn build_json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "reasoning": {
                    "type": "string",
                    "description": "分析问题原因和优化思路"
                },
                "patches": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "target_field": {
                                "type": "string",
                                "enum": ["mnemonic", "examples", "etymology"]
                            },
                            "operation": {
                                "type": "string",
                                "enum": ["replace", "append"]
                            },
                            "new_value": {
                                "type": "object",
                                "description": "新内容"
                            },
                            "reasoning": {
                                "type": "string",
                                "description": "为什么这样优化"
                            },
                            "confidence": {
                                "type": "number",
                                "minimum": 0.0,
                                "maximum": 1.0
                            }
                        },
                        "required": ["target_field", "operation", "new_value", "reasoning", "confidence"]
                    }
                }
            },
            "required": ["reasoning", "patches"]
        })
    }

    /// 转换为 CardPatch
    fn convert_to_patches(&self, result: OptimizationResult, model: &str) -> Vec<CardPatch> {
        result
            .patches
            .into_iter()
            .map(|p| {
                let operation = match p.operation.as_str() {
                    "replace" => PatchOperation::Replace,
                    "append" => PatchOperation::Append,
                    _ => PatchOperation::Replace,
                };

                CardPatch {
                    patch_id: uuid::Uuid::new_v4().to_string(),
                    target_field: p.target_field,
                    operation,
                    proposed_value: p.new_value,
                    reasoning: p.reasoning,
                    confidence: p.confidence as f32,
                    generated_by: model.to_string(),
                }
            })
            .collect()
    }
}

#[async_trait]
impl Skill for OptimizeCardSkill {
    fn name(&self) -> &str {
        "optimize_card"
    }

    fn description(&self) -> &str {
        "分析学习困难，使用 AI 优化卡牌内容"
    }

    async fn execute(&self, input: SkillInput) -> Result<SkillOutput> {
        // 解析上下文
        let context: OptimizeContext = if let Some(ctx) = input.get_param("context") {
            serde_json::from_value(ctx.clone())?
        } else {
            anyhow::bail!("缺少优化上下文");
        };

        // 构建 LLM 请求
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(&context);
        let json_schema = self.build_json_schema();

        let request = LlmRequest::new(vec![
            LlmMessage::system(system_prompt),
            LlmMessage::user(user_prompt),
        ])
        .with_temperature(0.8)
        .with_max_tokens(2000)
        .with_json_schema(json_schema);

        // 调用 LLM
        let response = self.provider.complete(request).await?;

        // 解析响应
        let result: OptimizationResult = serde_json::from_str(&response.content)?;

        // 转换为 Patches
        let patches = self.convert_to_patches(result.clone(), &response.model);

        // 返回结果
        Ok(SkillOutput::from_json(&patches)?
            .with_metadata("model", serde_json::json!(response.model))
            .with_metadata("reasoning", serde_json::json!(result.reasoning))
            .with_metadata("patch_count", serde_json::json!(patches.len()))
            .with_metadata("tokens", serde_json::json!(response.usage.total_tokens)))
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        if input.get_param("context").is_none() {
            anyhow::bail!("缺少优化上下文");
        }
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.provider.is_available()
    }
}

#[cfg(test)]
mod tests {
    use super::super::llm_provider::OpenAiCompatibleProvider;
    use super::*;

    #[test]
    fn test_build_prompts() {
        let provider = Arc::new(OpenAiCompatibleProvider::openai(
            "test".to_string(),
            "gpt-4".to_string(),
        ));
        let skill = OptimizeCardSkill::new(provider);

        let context = OptimizeContext {
            word: "brilliant".to_string(),
            current_content: "助记法: brill = 闪耀".to_string(),
            triggers: vec![OptimizeTrigger::LowRating { score: 2.0 }],
            error_history: None,
        };

        let system = skill.build_system_prompt();
        assert!(system.contains("优化助手"));

        let user = skill.build_user_prompt(&context);
        assert!(user.contains("brilliant"));
        assert!(user.contains("打分过低"));
    }
}

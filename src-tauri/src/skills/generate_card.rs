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
    pub pos: Option<String>,      // 词性
    pub phonetic: Option<String>, // 音标
    pub frequency: Option<i32>,   // 词频
}

/// AI 生成的原始输出（匹配 JSON Schema）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiGeneratedContent {
    etymology: Option<EtymologySchema>,
    mnemonics: Vec<MnemonicSchema>,
    examples: Vec<ExampleSchema>,
    collocations: Vec<String>,                       // 常见搭配
    word_family: Vec<crate::domain::WordFamilyItem>, // 词族
    usage_tips: Vec<String>,                         // 用法提示
    common_mistakes: Vec<String>,                    // 常见错误
    synonyms: Vec<String>,                           // 近义词
    antonyms: Vec<String>,                           // 反义词
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
        r"你是一个专业的英语学习内容生成助手，拥有语言学博士学位和十年教学经验。你的任务是为英语单词生成高质量、精细化的学习内容。

## 核心原则

1. **准确性第一**：词源分析必须基于真实语言学知识，不可编造
2. **实用性优先**：助记法要真正帮助记忆，不要牵强附会
3. **场景化例句**：例句要地道、现代、贴近真实生活场景
4. **层次分明**：从基础到进阶，满足不同水平学习者需求
5. **文化融入**：适当融入英语文化背景，加深理解

## 内容要求

### 词源分析
- 提供准确的词源（拉丁语/希腊语/法语/古英语等）
- 拆解词根、前缀、后缀，说明含义
- 列举同根词（至少3个），帮助举一反三

### 助记法（3-4种不同类型）
- **词根词缀法**：基于词源的记忆方法
- **场景联想法**：创造生动的画面或故事
- **谐音法**：利用中文谐音（如果合适）
- **词族法**：通过相关词群记忆
- **对比法**：与易混词对比区分

### 例句（3个，不同难度）
- **基础**：简单句，日常生活场景
- **中级**：复合句，工作/学习场景
- **高级**：复杂句，学术/专业场景
每个例句都要自然流畅，不要为了用词而造句

### 搭配与用法
- 列出最常见的3-5个搭配（形容词+名词、动词+介词等）
- 说明常见的语法结构
- 标注正式/非正式用法

### 词族扩展
- 列出同根的不同词性形式
- 简要说明每个形式的含义和用法

### 常见错误
- 中国学生容易犯的错误
- 与易混词的区别
- 常见的搭配错误

输出格式：严格按照 JSON Schema 输出，确保 JSON 格式正确。"
            .to_string()
    }

    /// 构建用户提示
    fn build_user_prompt(&self, context: &CardContext) -> String {
        let mut prompt = format!("## 目标单词\n\n**{}**\n\n", context.word);

        if let Some(phonetic) = &context.phonetic {
            prompt.push_str(&format!("**音标**: {phonetic}\n"));
        }

        if let Some(pos) = &context.pos {
            prompt.push_str(&format!("**词性**: {pos}\n"));
        }

        if let Some(freq) = &context.frequency {
            let level = if *freq <= 1000 {
                "核心高频词"
            } else if *freq <= 3000 {
                "常用词"
            } else if *freq <= 5000 {
                "中级词汇"
            } else {
                "进阶词汇"
            };
            prompt.push_str(&format!("**词频**: {freq}（{level}）\n"));
        }

        prompt.push('\n');

        if let Some(def) = &context.definition {
            prompt.push_str(&format!("**英文定义**: {def}\n"));
        }

        if let Some(trans) = &context.translation {
            prompt.push_str(&format!("**中文释义**: {trans}\n"));
        }

        if let Some(morph) = &context.morphology {
            prompt.push_str(&format!("**词根拆解**: {morph}\n"));
        }

        prompt.push_str("\n## 请生成以下内容\n\n");
        prompt.push_str("1. **词源分析**（如果有明确词源）\n");
        prompt.push_str("2. **助记法**（3-4种不同类型，确保实用）\n");
        prompt.push_str("3. **例句**（3个不同难度：基础/中级/高级）\n");
        prompt.push_str("4. **常见搭配**（3-5个高频搭配）\n");
        prompt.push_str("5. **词族**（同根的不同词性形式）\n");
        prompt.push_str("6. **用法提示**（2-3条实用建议）\n");
        prompt.push_str("7. **常见错误**（1-2个易犯错误）\n");
        prompt.push_str("8. **近义词/反义词**（各2-3个）\n\n");
        prompt.push_str("请确保内容准确、实用、有深度。");

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
                        "origin": {
                            "type": "string",
                            "description": "词源说明（如：来自拉丁语 xxx）"
                        },
                        "roots": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "part": { "type": "string", "description": "词根/前缀/后缀" },
                                    "meaning": { "type": "string", "description": "含义" },
                                    "examples": {
                                        "type": "array",
                                        "items": { "type": "string" },
                                        "description": "同根词示例（至少3个）"
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
                                "enum": ["etymology", "scene", "homophone", "visual", "chunking", "comparison"],
                                "description": "助记法类型"
                            },
                            "content": {
                                "type": "string",
                                "description": "助记法内容（要生动、实用）"
                            }
                        },
                        "required": ["type", "content"]
                    },
                    "minItems": 3,
                    "maxItems": 4
                },
                "examples": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "description": "英文例句" },
                            "context": { "type": "string", "description": "使用场景说明（中文）" },
                            "difficulty": {
                                "type": "string",
                                "enum": ["basic", "intermediate", "advanced"],
                                "description": "难度级别"
                            }
                        },
                        "required": ["text", "context", "difficulty"]
                    },
                    "minItems": 3
                },
                "collocations": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "常见搭配（3-5个），如：make a decision, heavy rain"
                },
                "word_family": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "word": { "type": "string" },
                            "pos": { "type": "string", "description": "词性（n./v./adj./adv.）" },
                            "meaning": { "type": "string", "description": "简要中文释义" }
                        },
                        "required": ["word", "pos", "meaning"]
                    },
                    "description": "词族（同根的不同词性形式）"
                },
                "usage_tips": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "用法提示（2-3条实用建议）"
                },
                "common_mistakes": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "常见错误（1-2个易犯错误及纠正）"
                },
                "synonyms": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "近义词（2-3个，简要说明区别）"
                },
                "antonyms": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "反义词（2-3个）"
                }
            },
            "required": ["mnemonics", "examples", "collocations", "word_family", "usage_tips"]
        })
    }

    /// 转换 AI 输出为 `AiContent`
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
                    "comparison" => MnemonicType::Comparison,
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

        let word_family = generated.word_family;

        AiContent {
            etymology,
            mnemonics,
            examples,
            scenes: vec![],
            collocations: generated.collocations,
            word_family,
            usage_tips: generated.usage_tips,
            common_mistakes: generated.common_mistakes,
            synonyms: generated.synonyms,
            antonyms: generated.antonyms,
        }
    }
}

#[async_trait]
impl Skill for GenerateCardSkill {
    fn name(&self) -> &'static str {
        "generate_card"
    }

    fn description(&self) -> &'static str {
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
                pos: None,
                phonetic: None,
                frequency: None,
            }
        };

        // 构建 LLM 请求
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(&context);
        let json_schema = self.build_json_schema();

        // 调用 LLM
        tracing::info!(
            "🤖 AI生成开始: word='{}', model='{}'",
            context.word,
            "current"
        );

        // 失败自动重试一次（降低温度以获得更稳定的 JSON 输出）
        let mut response = None;
        let mut last_err: Option<anyhow::Error> = None;

        for (attempt, temperature) in &[(0, 0.7f32), (1, 0.3f32)] {
            let request = LlmRequest::new(vec![
                LlmMessage::system(system_prompt.clone()),
                LlmMessage::user(user_prompt.clone()),
            ])
            .with_temperature(*temperature)
            .with_max_tokens(4000)
            .with_json_schema(json_schema.clone());

            match self.provider.complete(request).await {
                Ok(r) => {
                    let json_str = extract_json(&r.content);
                    if let Ok(parsed) = serde_json::from_str::<AiGeneratedContent>(json_str) {
                        tracing::info!(
                            "✅ AI生成成功: word='{}', attempt={}, tokens={}, content_len={}",
                            context.word,
                            attempt,
                            r.usage.total_tokens,
                            r.content.len()
                        );
                        response = Some((r, parsed));
                        break;
                    }
                    // P0 修复:用 chars().take() 避免在 UTF-8 多字节字符中间切断 panic
                    let preview: String = r.content.chars().take(200).collect();
                    tracing::warn!(
                        "❌ AI JSON解析失败: word='{}', attempt={}, 将重试, raw_content={}",
                        context.word,
                        attempt,
                        preview
                    );
                    last_err = Some(anyhow::anyhow!(
                        "AI 返回 JSON 解析失败: 原始内容: {preview}"
                    ));
                }
                Err(e) => {
                    tracing::warn!(
                        "❌ AI生成失败: word='{}', attempt={}, error={}",
                        context.word,
                        attempt,
                        e
                    );
                    last_err = Some(e);
                }
            }
        }

        let (response, generated) = match response {
            Some((r, parsed)) => (r, parsed),
            None => return Err(last_err.unwrap_or_else(|| anyhow::anyhow!("AI 生成失败"))),
        };

        tracing::info!(
            "✅ AI内容解析成功: word='{}', mnemonics={}, examples={}, collocations={}",
            context.word,
            generated.mnemonics.len(),
            generated.examples.len(),
            generated.collocations.len()
        );

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

/// 从 LLM 响应中提取 JSON（处理 markdown 代码块等包裹）
fn extract_json(content: &str) -> &str {
    let trimmed = content.trim();

    // 1. 如果整个内容就是 JSON
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return trimmed;
    }

    // 2. 提取 ```json ... ``` 代码块
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }

    // 3. 提取 ``` ... ``` 代码块
    if let Some(start) = trimmed.find("```") {
        let json_start = start + 3;
        // 跳过语言标识符行
        let after_marker = &trimmed[json_start..];
        let json_start = if let Some(newline) = after_marker.find('\n') {
            json_start + newline + 1
        } else {
            json_start
        };
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }

    // 4. 找第一个 { 到最后一个 }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                return &trimmed[start..=end];
            }
        }
    }

    trimmed
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
            pos: Some("adj.".to_string()),
            phonetic: Some("/ˈbrɪl.li.ənt/".to_string()),
            frequency: Some(1500),
        };

        let system = skill.build_system_prompt();
        assert!(system.contains("学习内容"));

        let user = skill.build_user_prompt(&context);
        assert!(user.contains("brilliant"));
        assert!(user.contains("very bright"));
        assert!(user.contains("adj."));
    }
}

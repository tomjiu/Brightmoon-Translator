// Skill System - 可扩展的技能系统
// Skills 是独立的、可组合的能力单元

pub mod generate_card;
pub mod llm_provider;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

pub use generate_card::GenerateCardSkill;
pub use llm_provider::{
    LlmMessage, LlmProvider, LlmRequest, LlmResponse, OpenAiCompatibleProvider,
};

/// Skill 输入
#[derive(Debug, Clone)]
pub struct SkillInput {
    /// 主要参数（如单词）
    pub primary: String,
    /// 额外参数
    pub params: HashMap<String, Value>,
}

impl SkillInput {
    /// 创建简单输入
    pub fn new(primary: impl Into<String>) -> Self {
        Self {
            primary: primary.into(),
            params: HashMap::new(),
        }
    }

    /// 添加参数
    pub fn with_param(mut self, key: impl Into<String>, value: Value) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    /// 获取参数
    pub fn get_param(&self, key: &str) -> Option<&Value> {
        self.params.get(key)
    }
}

/// Skill 输出
#[derive(Debug, Clone)]
pub struct SkillOutput {
    /// 结构化数据
    pub data: Value,
    /// 元数据（如来源、置信度）
    pub metadata: HashMap<String, Value>,
}

impl SkillOutput {
    /// 创建输出
    pub fn new(data: Value) -> Self {
        Self {
            data,
            metadata: HashMap::new(),
        }
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// 从 JSON 序列化对象创建
    pub fn from_json<T: serde::Serialize>(value: &T) -> Result<Self> {
        let data = serde_json::to_value(value)?;
        Ok(Self::new(data))
    }

    /// 反序列化为类型
    pub fn into_type<T: serde::de::DeserializeOwned>(self) -> Result<T> {
        Ok(serde_json::from_value(self.data)?)
    }
}

/// Skill Trait - 所有技能的抽象
#[async_trait]
pub trait Skill: Send + Sync {
    /// Skill 名称（唯一标识）
    fn name(&self) -> &str;

    /// Skill 描述
    fn description(&self) -> &str;

    /// 执行 Skill
    async fn execute(&self, input: SkillInput) -> Result<SkillOutput>;

    /// 验证输入（可选）
    fn validate_input(&self, _input: &SkillInput) -> Result<()> {
        Ok(())
    }

    /// 是否可用（可选，用于检查依赖）
    fn is_available(&self) -> bool {
        true
    }
}

/// Skill 包装器（带优先级）
pub struct SkillWrapper {
    pub skill: Box<dyn Skill>,
    pub priority: i32,
}

/// Skill Registry - 技能注册表
pub struct SkillRegistry {
    skills: HashMap<String, SkillWrapper>,
}

impl SkillRegistry {
    /// 创建空注册表
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// 注册技能
    pub fn register(&mut self, skill: Box<dyn Skill>, priority: i32) -> Result<()> {
        let name = skill.name().to_string();

        if self.skills.contains_key(&name) {
            anyhow::bail!("Skill '{}' already registered", name);
        }

        self.skills
            .insert(name.clone(), SkillWrapper { skill, priority });

        Ok(())
    }

    /// 获取技能
    pub fn get(&self, name: &str) -> Option<&Box<dyn Skill>> {
        self.skills.get(name).map(|w| &w.skill)
    }

    /// 执行技能
    pub async fn execute(&self, name: &str, input: SkillInput) -> Result<SkillOutput> {
        let skill = self
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

        // 验证输入
        skill.validate_input(&input)?;

        // 检查可用性
        if !skill.is_available() {
            anyhow::bail!("Skill '{}' is not available", name);
        }

        // 执行
        skill.execute(input).await
    }

    /// 列出所有技能
    pub fn list(&self) -> Vec<SkillInfo> {
        let mut skills: Vec<_> = self
            .skills
            .values()
            .map(|w| SkillInfo {
                name: w.skill.name().to_string(),
                description: w.skill.description().to_string(),
                priority: w.priority,
                available: w.skill.is_available(),
            })
            .collect();

        // 按优先级排序
        skills.sort_by(|a, b| b.priority.cmp(&a.priority));
        skills
    }

    /// 查找技能（按标签、模糊匹配等）
    pub fn find(&self, query: &str) -> Vec<SkillInfo> {
        let query_lower = query.to_lowercase();
        self.list()
            .into_iter()
            .filter(|info| {
                info.name.to_lowercase().contains(&query_lower)
                    || info.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Skill 信息
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub priority: i32,
    pub available: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSkill {
        name: String,
    }

    #[async_trait]
    impl Skill for MockSkill {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Mock skill for testing"
        }

        async fn execute(&self, input: SkillInput) -> Result<SkillOutput> {
            Ok(SkillOutput::new(serde_json::json!({
                "echo": input.primary
            })))
        }
    }

    #[tokio::test]
    async fn test_registry_register_and_execute() {
        let mut registry = SkillRegistry::new();

        let skill = Box::new(MockSkill {
            name: "test_skill".to_string(),
        });

        registry.register(skill, 100).unwrap();

        let input = SkillInput::new("hello");
        let output = registry.execute("test_skill", input).await.unwrap();

        assert_eq!(output.data["echo"], "hello");
    }

    #[tokio::test]
    async fn test_registry_list() {
        let mut registry = SkillRegistry::new();

        registry
            .register(
                Box::new(MockSkill {
                    name: "skill1".to_string(),
                }),
                100,
            )
            .unwrap();

        registry
            .register(
                Box::new(MockSkill {
                    name: "skill2".to_string(),
                }),
                50,
            )
            .unwrap();

        let skills = registry.list();
        assert_eq!(skills.len(), 2);
        // 优先级高的在前
        assert_eq!(skills[0].name, "skill1");
        assert_eq!(skills[1].name, "skill2");
    }

    #[tokio::test]
    async fn test_registry_find() {
        let mut registry = SkillRegistry::new();

        registry
            .register(
                Box::new(MockSkill {
                    name: "dict_lookup".to_string(),
                }),
                100,
            )
            .unwrap();

        let results = registry.find("dict");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "dict_lookup");
    }
}

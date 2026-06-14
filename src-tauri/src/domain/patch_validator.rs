// Patch Validator - 验证 AI 生成的 Patch

use crate::domain::{CardPatch, PatchOperation, WordCard};
use anyhow::{bail, Result};
use serde_json::Value;

/// Patch 验证错误
#[derive(Debug, thiserror::Error)]
pub enum PatchValidationError {
    #[error("目标字段不存在: {0}")]
    InvalidField(String),

    #[error("字段类型不匹配: 期望 {expected}, 实际 {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("索引越界: 索引 {index}, 长度 {length}")]
    IndexOutOfBounds { index: usize, length: usize },

    #[error("值验证失败: {0}")]
    InvalidValue(String),

    #[error("置信度过低: {0} (最小要求: {1})")]
    LowConfidence(f32, f32),

    #[error("Patch 冲突: {0}")]
    Conflict(String),
}

/// Patch 验证器
pub struct PatchValidator {
    /// 最小置信度阈值
    min_confidence: f32,
}

impl PatchValidator {
    /// 创建验证器
    pub fn new(min_confidence: f32) -> Self {
        Self { min_confidence }
    }

    /// 默认验证器（置信度 >= 0.7）
    pub fn default() -> Self {
        Self::new(0.7)
    }

    /// 验证 Patch
    pub fn validate(&self, patch: &CardPatch, card: &WordCard) -> Result<()> {
        // 1. 验证置信度
        self.validate_confidence(patch)?;

        // 2. 验证字段存在
        self.validate_field_exists(&patch.target_field)?;

        // 3. 验证操作合法性
        self.validate_operation(patch, card)?;

        // 4. 验证值类型
        self.validate_value_type(patch)?;

        // 5. 验证值合理性
        self.validate_value_content(patch)?;

        Ok(())
    }

    /// 验证置信度
    fn validate_confidence(&self, patch: &CardPatch) -> Result<()> {
        if patch.confidence < self.min_confidence {
            bail!(PatchValidationError::LowConfidence(
                patch.confidence,
                self.min_confidence
            ));
        }
        Ok(())
    }

    /// 验证字段存在
    fn validate_field_exists(&self, field: &str) -> Result<()> {
        const VALID_FIELDS: &[&str] = &[
            "mnemonic",
            "mnemonics",
            "etymology",
            "examples",
            "example",
            "scenes",
            "scene",
        ];

        if !VALID_FIELDS.contains(&field) {
            bail!(PatchValidationError::InvalidField(field.to_string()));
        }

        Ok(())
    }

    /// 验证操作合法性
    fn validate_operation(&self, patch: &CardPatch, card: &WordCard) -> Result<()> {
        match &patch.operation {
            PatchOperation::Replace => {
                // Replace 总是合法的
                Ok(())
            },
            PatchOperation::Append => {
                // Append 要求字段是数组类型
                if !self.is_array_field(&patch.target_field) {
                    bail!(PatchValidationError::InvalidValue(format!(
                        "字段 '{}' 不支持 Append 操作（非数组类型）",
                        patch.target_field
                    )));
                }
                Ok(())
            },
            PatchOperation::Update { index } => {
                // Update 要求索引有效
                let length = self.get_array_length(&patch.target_field, card)?;
                if *index >= length {
                    bail!(PatchValidationError::IndexOutOfBounds {
                        index: *index,
                        length
                    });
                }
                Ok(())
            },
        }
    }

    /// 验证值类型
    fn validate_value_type(&self, patch: &CardPatch) -> Result<()> {
        match patch.target_field.as_str() {
            "mnemonic" => {
                // 单个助记法：应该是对象
                if !patch.proposed_value.is_object() {
                    bail!(PatchValidationError::TypeMismatch {
                        expected: "Object (Mnemonic)".to_string(),
                        actual: value_type_name(&patch.proposed_value),
                    });
                }
            },
            "mnemonics" => {
                // 多个助记法：应该是数组
                if !patch.proposed_value.is_array() {
                    bail!(PatchValidationError::TypeMismatch {
                        expected: "Array[Mnemonic]".to_string(),
                        actual: value_type_name(&patch.proposed_value),
                    });
                }
            },
            "etymology" => {
                // 词源：应该是对象
                if !patch.proposed_value.is_object() && !patch.proposed_value.is_null() {
                    bail!(PatchValidationError::TypeMismatch {
                        expected: "Object (Etymology) or null".to_string(),
                        actual: value_type_name(&patch.proposed_value),
                    });
                }
            },
            "examples" | "example" => {
                // 例句：数组或单个对象
                if patch.target_field == "examples" && !patch.proposed_value.is_array() {
                    bail!(PatchValidationError::TypeMismatch {
                        expected: "Array[Example]".to_string(),
                        actual: value_type_name(&patch.proposed_value),
                    });
                }
                if patch.target_field == "example" && !patch.proposed_value.is_object() {
                    bail!(PatchValidationError::TypeMismatch {
                        expected: "Object (Example)".to_string(),
                        actual: value_type_name(&patch.proposed_value),
                    });
                }
            },
            "scenes" | "scene" => {
                // 场景：数组或单个对象
                if patch.target_field == "scenes" && !patch.proposed_value.is_array() {
                    bail!(PatchValidationError::TypeMismatch {
                        expected: "Array[Scene]".to_string(),
                        actual: value_type_name(&patch.proposed_value),
                    });
                }
                if patch.target_field == "scene" && !patch.proposed_value.is_object() {
                    bail!(PatchValidationError::TypeMismatch {
                        expected: "Object (Scene)".to_string(),
                        actual: value_type_name(&patch.proposed_value),
                    });
                }
            },
            _ => {},
        }

        Ok(())
    }

    /// 验证值内容合理性
    fn validate_value_content(&self, patch: &CardPatch) -> Result<()> {
        match patch.target_field.as_str() {
            "mnemonic" | "mnemonics" => {
                self.validate_mnemonic_content(&patch.proposed_value)?;
            },
            "etymology" => {
                self.validate_etymology_content(&patch.proposed_value)?;
            },
            "examples" | "example" => {
                self.validate_example_content(&patch.proposed_value)?;
            },
            _ => {},
        }

        Ok(())
    }

    /// 验证助记法内容
    fn validate_mnemonic_content(&self, value: &Value) -> Result<()> {
        let mnemonics = if value.is_array() {
            value.as_array().unwrap()
        } else {
            &vec![value.clone()]
        };

        for mnemonic in mnemonics {
            // 检查必需字段
            let content = mnemonic
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    PatchValidationError::InvalidValue("助记法缺少 content 字段".to_string())
                })?;

            // 内容长度检查
            if content.is_empty() {
                bail!(PatchValidationError::InvalidValue(
                    "助记法内容不能为空".to_string()
                ));
            }

            if content.len() > 500 {
                bail!(PatchValidationError::InvalidValue(
                    "助记法内容过长（最多500字符）".to_string()
                ));
            }

            // 检查类型字段
            if let Some(mnemonic_type) = mnemonic.get("mnemonic_type") {
                let valid_types = &["etymology", "scene", "homophone", "visual", "chunking"];
                if let Some(type_str) = mnemonic_type.as_str() {
                    if !valid_types.contains(&type_str) {
                        bail!(PatchValidationError::InvalidValue(format!(
                            "无效的助记法类型: {}",
                            type_str
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// 验证词源内容
    fn validate_etymology_content(&self, value: &Value) -> Result<()> {
        if value.is_null() {
            return Ok(());
        }

        let origin = value
            .get("origin")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PatchValidationError::InvalidValue("词源缺少 origin 字段".to_string())
            })?;

        if origin.is_empty() {
            bail!(PatchValidationError::InvalidValue(
                "词源 origin 不能为空".to_string()
            ));
        }

        Ok(())
    }

    /// 验证例句内容
    fn validate_example_content(&self, value: &Value) -> Result<()> {
        let examples = if value.is_array() {
            value.as_array().unwrap()
        } else {
            &vec![value.clone()]
        };

        for example in examples {
            let text = example
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    PatchValidationError::InvalidValue("例句缺少 text 字段".to_string())
                })?;

            if text.is_empty() {
                bail!(PatchValidationError::InvalidValue(
                    "例句内容不能为空".to_string()
                ));
            }

            if text.len() > 300 {
                bail!(PatchValidationError::InvalidValue(
                    "例句过长（最多300字符）".to_string()
                ));
            }
        }

        Ok(())
    }

    /// 检查是否是数组字段
    fn is_array_field(&self, field: &str) -> bool {
        matches!(field, "mnemonics" | "examples" | "scenes")
    }

    /// 获取数组长度
    fn get_array_length(&self, field: &str, card: &WordCard) -> Result<usize> {
        let ai_content = card
            .ai_content
            .as_ref()
            .ok_or_else(|| PatchValidationError::InvalidValue("卡牌无AI内容".to_string()))?;

        let length = match field {
            "mnemonics" => ai_content.mnemonics.len(),
            "examples" => ai_content.examples.len(),
            "scenes" => ai_content.scenes.len(),
            _ => 0,
        };

        Ok(length)
    }
}

/// 获取 JSON 值类型名称
fn value_type_name(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(_) => "boolean".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::String(_) => "string".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AiContent, BaseData, CardPatch, Mnemonic, MnemonicType, PatchOperation};

    #[test]
    fn test_validate_confidence() {
        let validator = PatchValidator::new(0.7);

        let high_confidence_patch = CardPatch {
            patch_id: "test".to_string(),
            target_field: "mnemonic".to_string(),
            operation: PatchOperation::Replace,
            proposed_value: serde_json::json!({}),
            reasoning: "test".to_string(),
            confidence: 0.9,
            generated_by: "gpt-4".to_string(),
        };

        let card = create_test_card();
        assert!(validator.validate(&high_confidence_patch, &card).is_ok());

        let low_confidence_patch = CardPatch {
            confidence: 0.5,
            ..high_confidence_patch
        };

        assert!(validator.validate(&low_confidence_patch, &card).is_err());
    }

    #[test]
    fn test_validate_invalid_field() {
        let validator = PatchValidator::default();

        let patch = CardPatch {
            patch_id: "test".to_string(),
            target_field: "invalid_field".to_string(),
            operation: PatchOperation::Replace,
            proposed_value: serde_json::json!({}),
            reasoning: "test".to_string(),
            confidence: 0.9,
            generated_by: "gpt-4".to_string(),
        };

        let card = create_test_card();
        assert!(validator.validate(&patch, &card).is_err());
    }

    fn create_test_card() -> WordCard {
        WordCard {
            id: "test".to_string(),
            word: "test".to_string(),
            current_version: 1,
            base_data: BaseData {
                phonetic: None,
                part_of_speech: None,
                definitions: vec![],
                translation: None,
            },
            ai_content: Some(AiContent {
                etymology: None,
                mnemonics: vec![Mnemonic {
                    mnemonic_type: MnemonicType::Etymology,
                    content: "test".to_string(),
                    score: None,
                }],
                examples: vec![],
                scenes: vec![],
            }),
            fsrs_state: Default::default(),
            error_records: vec![],
            annotations: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }
}

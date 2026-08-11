// Patch Applicator - 应用已验证的 Patch

use crate::domain::{
    AiContent, CardPatch, Mnemonic, PatchOperation, PersonalizedExample, Scene, WordCard,
};
use anyhow::{bail, Context, Result};

/// Patch 应用器
pub struct PatchApplicator;

impl PatchApplicator {
    /// 应用 Patch 到卡牌
    pub fn apply(patch: &CardPatch, card: &mut WordCard) -> Result<()> {
        // 确保卡牌有 AI 内容
        if card.ai_content.is_none() {
            card.ai_content = Some(AiContent {
                etymology: None,
                mnemonics: vec![],
                examples: vec![],
                scenes: vec![],
                collocations: vec![],
                word_family: vec![],
                usage_tips: vec![],
                common_mistakes: vec![],
                synonyms: vec![],
                antonyms: vec![],
            });
        }

        let ai_content = card.ai_content.as_mut().unwrap();

        // 根据目标字段应用 Patch
        match patch.target_field.as_str() {
            "mnemonic" => Self::apply_single_mnemonic(patch, ai_content)?,
            "mnemonics" => Self::apply_mnemonics(patch, ai_content)?,
            "etymology" => Self::apply_etymology(patch, ai_content)?,
            "example" => Self::apply_single_example(patch, ai_content)?,
            "examples" => Self::apply_examples(patch, ai_content)?,
            "scene" => Self::apply_single_scene(patch, ai_content)?,
            "scenes" => Self::apply_scenes(patch, ai_content)?,
            _ => bail!("不支持的字段: {}", patch.target_field),
        }

        Ok(())
    }

    /// 应用单个助记法
    fn apply_single_mnemonic(patch: &CardPatch, content: &mut AiContent) -> Result<()> {
        let mnemonic: Mnemonic =
            serde_json::from_value(patch.proposed_value.clone()).context("反序列化助记法失败")?;

        match &patch.operation {
            PatchOperation::Replace => {
                // 替换第一个助记法，如果没有则添加
                if content.mnemonics.is_empty() {
                    content.mnemonics.push(mnemonic);
                } else {
                    content.mnemonics[0] = mnemonic;
                }
            },
            PatchOperation::Append => {
                content.mnemonics.push(mnemonic);
            },
            PatchOperation::Update { index } => {
                if *index >= content.mnemonics.len() {
                    bail!("索引越界: {index}");
                }
                content.mnemonics[*index] = mnemonic;
            },
        }

        Ok(())
    }

    /// 应用多个助记法
    fn apply_mnemonics(patch: &CardPatch, content: &mut AiContent) -> Result<()> {
        let mnemonics: Vec<Mnemonic> = serde_json::from_value(patch.proposed_value.clone())
            .context("反序列化助记法列表失败")?;

        match &patch.operation {
            PatchOperation::Replace => {
                content.mnemonics = mnemonics;
            },
            PatchOperation::Append => {
                content.mnemonics.extend(mnemonics);
            },
            PatchOperation::Update { .. } => {
                bail!("Update 操作不支持数组字段，请使用单个元素");
            },
        }

        Ok(())
    }

    /// 应用词源
    fn apply_etymology(patch: &CardPatch, content: &mut AiContent) -> Result<()> {
        let etymology = if patch.proposed_value.is_null() {
            None
        } else {
            Some(serde_json::from_value(patch.proposed_value.clone()).context("反序列化词源失败")?)
        };

        content.etymology = etymology;
        Ok(())
    }

    /// 应用单个例句
    fn apply_single_example(patch: &CardPatch, content: &mut AiContent) -> Result<()> {
        let example: PersonalizedExample =
            serde_json::from_value(patch.proposed_value.clone()).context("反序列化例句失败")?;

        match &patch.operation {
            PatchOperation::Replace => {
                if content.examples.is_empty() {
                    content.examples.push(example);
                } else {
                    content.examples[0] = example;
                }
            },
            PatchOperation::Append => {
                content.examples.push(example);
            },
            PatchOperation::Update { index } => {
                if *index >= content.examples.len() {
                    bail!("索引越界: {index}");
                }
                content.examples[*index] = example;
            },
        }

        Ok(())
    }

    /// 应用多个例句
    fn apply_examples(patch: &CardPatch, content: &mut AiContent) -> Result<()> {
        let examples: Vec<PersonalizedExample> =
            serde_json::from_value(patch.proposed_value.clone()).context("反序列化例句列表失败")?;

        match &patch.operation {
            PatchOperation::Replace => {
                content.examples = examples;
            },
            PatchOperation::Append => {
                content.examples.extend(examples);
            },
            PatchOperation::Update { .. } => {
                bail!("Update 操作不支持数组字段，请使用单个元素");
            },
        }

        Ok(())
    }

    /// 应用单个场景
    fn apply_single_scene(patch: &CardPatch, content: &mut AiContent) -> Result<()> {
        let scene: Scene =
            serde_json::from_value(patch.proposed_value.clone()).context("反序列化场景失败")?;

        match &patch.operation {
            PatchOperation::Replace => {
                if content.scenes.is_empty() {
                    content.scenes.push(scene);
                } else {
                    content.scenes[0] = scene;
                }
            },
            PatchOperation::Append => {
                content.scenes.push(scene);
            },
            PatchOperation::Update { index } => {
                if *index >= content.scenes.len() {
                    bail!("索引越界: {index}");
                }
                content.scenes[*index] = scene;
            },
        }

        Ok(())
    }

    /// 应用多个场景
    fn apply_scenes(patch: &CardPatch, content: &mut AiContent) -> Result<()> {
        let scenes: Vec<Scene> =
            serde_json::from_value(patch.proposed_value.clone()).context("反序列化场景列表失败")?;

        match &patch.operation {
            PatchOperation::Replace => {
                content.scenes = scenes;
            },
            PatchOperation::Append => {
                content.scenes.extend(scenes);
            },
            PatchOperation::Update { .. } => {
                bail!("Update 操作不支持数组字段，请使用单个元素");
            },
        }

        Ok(())
    }

    /// 预览 Patch 效果（不实际修改卡牌）
    pub fn preview(patch: &CardPatch, card: &WordCard) -> Result<WordCard> {
        let mut cloned_card = card.clone();
        Self::apply(patch, &mut cloned_card)?;
        Ok(cloned_card)
    }

    /// 批量应用多个 Patch
    pub fn apply_batch(patches: &[CardPatch], card: &mut WordCard) -> Result<()> {
        for patch in patches {
            Self::apply(patch, card)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BaseData, Mnemonic, MnemonicType, PatchOperation};

    #[test]
    fn test_apply_mnemonic_replace() {
        let mut card = create_test_card();

        let patch = CardPatch {
            patch_id: "test".to_string(),
            target_field: "mnemonic".to_string(),
            operation: PatchOperation::Replace,
            proposed_value: serde_json::json!({
                "mnemonic_type": "etymology",
                "content": "新助记法",
                "score": null
            }),
            reasoning: "优化助记法".to_string(),
            confidence: 0.9,
            generated_by: "gpt-4".to_string(),
        };

        PatchApplicator::apply(&patch, &mut card).unwrap();

        assert_eq!(card.ai_content.as_ref().unwrap().mnemonics.len(), 1);
        assert_eq!(
            card.ai_content.as_ref().unwrap().mnemonics[0].content,
            "新助记法"
        );
    }

    #[test]
    fn test_apply_mnemonic_append() {
        let mut card = create_test_card();

        let patch = CardPatch {
            patch_id: "test".to_string(),
            target_field: "mnemonic".to_string(),
            operation: PatchOperation::Append,
            proposed_value: serde_json::json!({
                "mnemonic_type": "scene",
                "content": "场景助记法",
                "score": null
            }),
            reasoning: "添加场景助记".to_string(),
            confidence: 0.85,
            generated_by: "gpt-4".to_string(),
        };

        PatchApplicator::apply(&patch, &mut card).unwrap();

        assert_eq!(card.ai_content.as_ref().unwrap().mnemonics.len(), 2);
        assert_eq!(
            card.ai_content.as_ref().unwrap().mnemonics[1].content,
            "场景助记法"
        );
    }

    #[test]
    fn test_preview_patch() {
        let card = create_test_card();

        let patch = CardPatch {
            patch_id: "test".to_string(),
            target_field: "mnemonic".to_string(),
            operation: PatchOperation::Replace,
            proposed_value: serde_json::json!({
                "mnemonic_type": "etymology",
                "content": "预览助记法",
                "score": null
            }),
            reasoning: "预览".to_string(),
            confidence: 0.9,
            generated_by: "gpt-4".to_string(),
        };

        // 预览不修改原卡牌
        let previewed = PatchApplicator::preview(&patch, &card).unwrap();

        assert_eq!(
            card.ai_content.as_ref().unwrap().mnemonics[0].content,
            "原助记法"
        );
        assert_eq!(
            previewed.ai_content.as_ref().unwrap().mnemonics[0].content,
            "预览助记法"
        );
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
                    content: "原助记法".to_string(),
                    score: None,
                }],
                examples: vec![],
                scenes: vec![],
                collocations: vec![],
                word_family: vec![],
                usage_tips: vec![],
                common_mistakes: vec![],
                synonyms: vec![],
                antonyms: vec![],
            }),
            fsrs_state: crate::domain::CardState::default(),
            error_records: vec![],
            annotations: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }
}

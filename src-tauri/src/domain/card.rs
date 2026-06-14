// Word Card - 从事件流重放得到的卡牌状态

use super::event::{AiContent, Annotation, CardEvent, CardState};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 单词卡牌（派生状态，可从事件流重建）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordCard {
    pub id: String,
    pub word: String,
    pub current_version: u32,

    // 基础数据（从词典查询，不常变）
    pub base_data: BaseData,

    // AI 生成内容（当前版本）
    pub ai_content: Option<AiContent>,

    // FSRS 状态
    pub fsrs_state: CardState,

    // 错误记录
    pub error_records: Vec<ErrorRecord>,

    // AI 批注
    pub annotations: Vec<Annotation>,

    // 元数据
    pub created_at: i64,
    pub updated_at: i64,
}

/// 基础词典数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseData {
    pub phonetic: Option<String>,
    pub part_of_speech: Option<String>,
    pub definitions: Vec<String>,
    pub translation: Option<String>,
}

/// 错误记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub timestamp: i64,
    pub error_type: ErrorType,
    pub user_answer: String,
    pub correct_answer: String,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    Spelling,
    Meaning,
    Usage,
}

impl WordCard {
    /// 创建新卡牌
    pub fn new(id: String, word: String, base_data: BaseData) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id,
            word,
            current_version: 1,
            base_data,
            ai_content: None,
            fsrs_state: CardState::default(),
            error_records: Vec::new(),
            annotations: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 从事件流重建卡牌（Event Sourcing 核心）
    pub fn from_events(events: &[CardEvent]) -> Result<Self> {
        if events.is_empty() {
            anyhow::bail!("Cannot rebuild card from empty event list");
        }

        // 第一个事件必须是 WordImported
        let first_event = &events[0];
        let word = match first_event {
            CardEvent::WordImported { word, .. } => word.clone(),
            _ => anyhow::bail!("First event must be WordImported"),
        };

        // 创建初始卡牌
        let mut card = Self::new(
            uuid::Uuid::new_v4().to_string(),
            word.clone(),
            BaseData {
                phonetic: None,
                part_of_speech: None,
                definitions: Vec::new(),
                translation: None,
            },
        );

        // 重放所有事件
        for event in events {
            card.apply_event(event)?;
        }

        Ok(card)
    }

    /// 应用单个事件
    pub fn apply_event(&mut self, event: &CardEvent) -> Result<()> {
        match event {
            CardEvent::WordImported {
                word, timestamp, ..
            } => {
                self.word = word.clone();
                self.created_at = *timestamp;
                self.updated_at = *timestamp;
            },

            CardEvent::AiContentGenerated {
                content, timestamp, ..
            } => {
                self.ai_content = Some(content.clone());
                self.updated_at = *timestamp;
            },

            CardEvent::PatchApplied {
                version,
                patch,
                timestamp,
            } => {
                self.current_version = *version;

                // 应用 Patch 到对应字段
                if let Some(ref mut ai_content) = self.ai_content {
                    match patch.target_field.as_str() {
                        "mnemonic" => {
                            // 更新助记法
                            if let Ok(new_mnemonic) =
                                serde_json::from_value(patch.proposed_value.clone())
                            {
                                ai_content.mnemonics = new_mnemonic;
                            }
                        },
                        "examples" => {
                            if let Ok(new_examples) =
                                serde_json::from_value(patch.proposed_value.clone())
                            {
                                ai_content.examples = new_examples;
                            }
                        },
                        "etymology" => {
                            if let Ok(new_etymology) =
                                serde_json::from_value(patch.proposed_value.clone())
                            {
                                ai_content.etymology = new_etymology;
                            }
                        },
                        _ => {},
                    }
                }

                self.updated_at = *timestamp;
            },

            CardEvent::RolledBack {
                to_version,
                timestamp,
                ..
            } => {
                self.current_version = *to_version;
                self.updated_at = *timestamp;
                // 注意：实际回退需要重新从事件流重建到目标版本
            },

            CardEvent::QuizCompleted {
                correct,
                user_answer,
                correct_answer,
                timestamp,
                ..
            } => {
                if !correct {
                    self.error_records.push(ErrorRecord {
                        timestamp: *timestamp,
                        error_type: ErrorType::Spelling, // 根据实际情况判断
                        user_answer: user_answer.clone(),
                        correct_answer: correct_answer.clone(),
                        context: String::new(),
                    });
                }
                self.updated_at = *timestamp;
            },

            CardEvent::AnnotationGenerated {
                annotation,
                timestamp,
            } => {
                self.annotations.push(annotation.clone());
                self.updated_at = *timestamp;
            },

            CardEvent::FsrsUpdated {
                new_state,
                timestamp,
                ..
            } => {
                self.fsrs_state = new_state.clone();
                self.updated_at = *timestamp;
            },

            _ => {
                // 其他事件不直接修改卡牌状态
            },
        }

        Ok(())
    }

    /// 回退到指定版本
    pub fn rollback_to_version(events: &[CardEvent], target_version: u32) -> Result<Self> {
        // 只重放到目标版本的事件
        let target_events: Vec<_> = events
            .iter()
            .take_while(|e| {
                if let CardEvent::PatchApplied { version, .. } = e {
                    *version <= target_version
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        Self::from_events(&target_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{AiContent, CardEvent, Mnemonic, MnemonicType};

    #[test]
    fn test_card_from_events() {
        let events = vec![
            CardEvent::WordImported {
                word: "brilliant".to_string(),
                source: "manual".to_string(),
                timestamp: 1000,
            },
            CardEvent::AiContentGenerated {
                content: AiContent {
                    etymology: None,
                    mnemonics: vec![Mnemonic {
                        mnemonic_type: MnemonicType::Etymology,
                        content: "brill-(闪耀) + -iant".to_string(),
                        score: None,
                    }],
                    examples: Vec::new(),
                    scenes: Vec::new(),
                },
                model: "gpt-4".to_string(),
                confidence: 0.9,
                timestamp: 2000,
            },
        ];

        let card = WordCard::from_events(&events).unwrap();

        assert_eq!(card.word, "brilliant");
        assert_eq!(card.current_version, 1);
        assert!(card.ai_content.is_some());
        assert_eq!(card.created_at, 1000);
        assert_eq!(card.updated_at, 2000);
    }
}

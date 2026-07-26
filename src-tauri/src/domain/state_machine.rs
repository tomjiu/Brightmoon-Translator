// Learning State Machine - 学习状态机

use crate::domain::{CardEvent, CardState, WordCard};
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// 学习阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningPhase {
    /// 新词（未学习）
    New,
    /// 学习中（首次几次复习）
    Learning,
    /// 复习中（已掌握基础）
    Review,
    /// 已精通（长期记忆）
    Mastered,
}

/// 优化触发条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizeTrigger {
    /// 低分（< 3分）
    LowRating { score: f32 },
    /// 连续遗忘（Again 次数过多）
    FrequentLapses { lapses: u32 },
    /// 用户反馈
    UserFeedback { feedback: String },
    /// 错误记录
    ErrorDetected { error_type: String },
}

/// 学习状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningState {
    /// 当前阶段
    pub phase: LearningPhase,
    /// 需要优化
    pub needs_optimization: bool,
    /// 优化触发原因
    pub optimization_triggers: Vec<OptimizeTrigger>,
    /// 上次状态更新时间
    pub last_updated: i64,
}

impl LearningState {
    /// 创建新词状态
    pub fn new() -> Self {
        Self {
            phase: LearningPhase::New,
            needs_optimization: false,
            optimization_triggers: Vec::new(),
            last_updated: Utc::now().timestamp(),
        }
    }

    /// 从卡牌状态推断学习阶段
    pub fn from_card(card: &WordCard) -> Self {
        let phase = Self::infer_phase(&card.fsrs_state);
        Self {
            phase,
            needs_optimization: false,
            optimization_triggers: Vec::new(),
            last_updated: Utc::now().timestamp(),
        }
    }

    /// 推断学习阶段
    fn infer_phase(fsrs_state: &CardState) -> LearningPhase {
        match fsrs_state.reps {
            0 => LearningPhase::New,
            1..=3 => LearningPhase::Learning,
            4..=10 => {
                if fsrs_state.stability > 30.0 {
                    LearningPhase::Mastered
                } else {
                    LearningPhase::Review
                }
            },
            _ => {
                if fsrs_state.stability > 60.0 && fsrs_state.lapses < 3 {
                    LearningPhase::Mastered
                } else {
                    LearningPhase::Review
                }
            },
        }
    }

    /// 添加优化触发器
    pub fn add_trigger(&mut self, trigger: OptimizeTrigger) {
        self.optimization_triggers.push(trigger);
        self.needs_optimization = true;
        self.last_updated = Utc::now().timestamp();
    }

    /// 清除优化触发器
    pub fn clear_triggers(&mut self) {
        self.optimization_triggers.clear();
        self.needs_optimization = false;
        self.last_updated = Utc::now().timestamp();
    }
}

impl Default for LearningState {
    fn default() -> Self {
        Self::new()
    }
}

/// 状态机
pub struct StateMachine;

impl StateMachine {
    /// 处理事件，返回新的学习状态
    pub fn process_event(
        current_state: &LearningState,
        event: &CardEvent,
        card: &WordCard,
    ) -> Result<LearningState> {
        let mut new_state = current_state.clone();

        match event {
            CardEvent::WordImported { .. } => {
                // 导入新词，保持 New 阶段
                new_state.phase = LearningPhase::New;
            },

            CardEvent::AiContentGenerated { .. } => {
                // AI 生成内容，可以开始学习
                // 阶段不变，但清除优化触发器
                new_state.clear_triggers();
            },

            CardEvent::QuizCompleted { correct, .. } => {
                // 更新学习阶段
                new_state.phase = LearningState::infer_phase(&card.fsrs_state);

                // 检查是否需要优化
                if !correct {
                    new_state.add_trigger(OptimizeTrigger::ErrorDetected {
                        error_type: "quiz_incorrect".to_string(),
                    });

                    // 频繁遗忘检查
                    if card.fsrs_state.lapses >= 3 {
                        new_state.add_trigger(OptimizeTrigger::FrequentLapses {
                            lapses: card.fsrs_state.lapses,
                        });
                    }
                }
            },

            CardEvent::UserRated {
                score, field: _, ..
            } => {
                // 低分触发优化
                if *score < 3.0 {
                    new_state.add_trigger(OptimizeTrigger::LowRating { score: *score });
                }
            },

            CardEvent::OptimizationRequested { .. } => {
                // 优化请求已触发，保持需要优化状态
                new_state.needs_optimization = true;
            },

            CardEvent::PatchApplied { .. } => {
                // Patch 应用后，清除优化触发器
                new_state.clear_triggers();
            },

            _ => {
                // 其他事件不影响学习状态
            },
        }

        new_state.last_updated = Utc::now().timestamp();
        Ok(new_state)
    }

    /// 决定下一步行动
    pub fn next_action(state: &LearningState, card: &WordCard) -> NextAction {
        // 1. 优先处理优化需求
        if state.needs_optimization {
            return NextAction::Optimize {
                triggers: state.optimization_triggers.clone(),
            };
        }

        // 2. 根据学习阶段决定
        match state.phase {
            LearningPhase::New => {
                // 新词：先生成内容
                if card.ai_content.is_none() {
                    NextAction::GenerateContent
                } else {
                    NextAction::StartLearning
                }
            },

            LearningPhase::Learning | LearningPhase::Review => {
                // 学习/复习中：检查是否到期
                let fsrs = crate::domain::FsrsEngine::new();
                if fsrs.should_review(&card.fsrs_state) {
                    NextAction::Review {
                        overdue_days: fsrs.overdue_days(&card.fsrs_state),
                    }
                } else {
                    NextAction::Wait {
                        next_review: card.fsrs_state.next_review,
                    }
                }
            },

            LearningPhase::Mastered => {
                // 已精通：长期复习
                let fsrs = crate::domain::FsrsEngine::new();
                if fsrs.should_review(&card.fsrs_state) {
                    NextAction::Review {
                        overdue_days: fsrs.overdue_days(&card.fsrs_state),
                    }
                } else {
                    NextAction::Wait {
                        next_review: card.fsrs_state.next_review,
                    }
                }
            },
        }
    }

    /// 检查是否应该自动优化
    pub fn should_auto_optimize(state: &LearningState) -> bool {
        if !state.needs_optimization {
            return false;
        }

        // 检查触发器严重程度
        for trigger in &state.optimization_triggers {
            match trigger {
                OptimizeTrigger::FrequentLapses { lapses } => {
                    if *lapses >= 3 {
                        return true;
                    }
                },
                OptimizeTrigger::LowRating { score } => {
                    if *score <= 2.0 {
                        return true;
                    }
                },
                _ => {},
            }
        }

        false
    }
}

/// 下一步行动
#[derive(Debug, Clone)]
pub enum NextAction {
    /// 生成学习内容
    GenerateContent,
    /// 开始学习
    StartLearning,
    /// 复习
    Review { overdue_days: i64 },
    /// 等待
    Wait { next_review: i64 },
    /// 优化
    Optimize { triggers: Vec<OptimizeTrigger> },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::BaseData;

    #[test]
    fn test_learning_state_new() {
        let state = LearningState::new();
        assert_eq!(state.phase, LearningPhase::New);
        assert!(!state.needs_optimization);
    }

    #[test]
    fn test_infer_phase() {
        // 新词
        let mut fsrs_state = CardState::default();
        assert_eq!(LearningState::infer_phase(&fsrs_state), LearningPhase::New);

        // 学习中
        fsrs_state.reps = 2;
        assert_eq!(
            LearningState::infer_phase(&fsrs_state),
            LearningPhase::Learning
        );

        // 复习中
        fsrs_state.reps = 5;
        fsrs_state.stability = 20.0;
        assert_eq!(
            LearningState::infer_phase(&fsrs_state),
            LearningPhase::Review
        );

        // 已精通
        fsrs_state.reps = 15;
        fsrs_state.stability = 70.0;
        fsrs_state.lapses = 1;
        assert_eq!(
            LearningState::infer_phase(&fsrs_state),
            LearningPhase::Mastered
        );
    }

    #[test]
    fn test_add_trigger() {
        let mut state = LearningState::new();
        assert!(!state.needs_optimization);

        state.add_trigger(OptimizeTrigger::LowRating { score: 2.0 });
        assert!(state.needs_optimization);
        assert_eq!(state.optimization_triggers.len(), 1);
    }

    #[test]
    fn test_process_event_quiz_completed() {
        let state = LearningState::new();
        let mut card = create_test_card();
        card.fsrs_state.reps = 2;

        let event = CardEvent::QuizCompleted {
            correct: false,
            user_answer: "wrong".to_string(),
            correct_answer: "right".to_string(),
            time_spent: 5000,
            timestamp: Utc::now().timestamp(),
        };

        let new_state = StateMachine::process_event(&state, &event, &card).unwrap();
        assert!(new_state.needs_optimization);
        assert!(!new_state.optimization_triggers.is_empty());
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
            ai_content: None,
            fsrs_state: CardState::default(),
            error_records: vec![],
            annotations: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }
}

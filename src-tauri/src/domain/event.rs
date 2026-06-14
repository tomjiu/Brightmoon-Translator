// Event Sourcing - Card Event Definitions
// 所有卡牌变更都通过事件记录

use serde::{Deserialize, Serialize};

/// 卡牌事件类型枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CardEvent {
    /// 单词导入
    WordImported {
        word: String,
        source: String, // "wordbook" | "manual" | "browser_extension"
        timestamp: i64,
    },

    /// AI 分析请求
    AiAnalysisRequested { timestamp: i64 },

    /// AI 内容生成
    AiContentGenerated {
        content: AiContent,
        model: String, // "gpt-4" | "deepseek-chat"
        confidence: f32,
        timestamp: i64,
    },

    /// 优化请求
    OptimizationRequested {
        field: String,  // "mnemonic" | "examples" | "etymology"
        reason: String, // "low_rating" | "user_feedback" | "error_detected"
        timestamp: i64,
    },

    /// Patch 提议
    PatchProposed { patch: CardPatch, timestamp: i64 },

    /// Patch 应用
    PatchApplied {
        version: u32,
        patch: CardPatch,
        timestamp: i64,
    },

    /// 回退到历史版本
    RolledBack {
        to_version: u32,
        reason: Option<String>,
        timestamp: i64,
    },

    /// 用户打分
    UserRated {
        field: String, // "mnemonic" | "example_1" | "etymology"
        score: f32,    // 0.0 - 5.0
        feedback: Option<String>,
        timestamp: i64,
    },

    /// 测验开始
    QuizStarted {
        quiz_type: String, // "multiple_choice" | "spelling" | "fill_blank"
        timestamp: i64,
    },

    /// 测验完成
    QuizCompleted {
        correct: bool,
        user_answer: String,
        correct_answer: String,
        time_spent: u32, // 毫秒
        timestamp: i64,
    },

    /// 批注请求
    AnnotationRequested {
        trigger: String, // "after_error" | "low_rating" | "periodic"
        timestamp: i64,
    },

    /// 批注生成
    AnnotationGenerated {
        annotation: Annotation,
        timestamp: i64,
    },

    /// FSRS 状态更新
    FsrsUpdated {
        grade: Rating,
        new_state: CardState,
        timestamp: i64,
    },
}

impl CardEvent {
    /// 获取事件时间戳
    pub fn timestamp(&self) -> i64 {
        match self {
            Self::WordImported { timestamp, .. } => *timestamp,
            Self::AiAnalysisRequested { timestamp } => *timestamp,
            Self::AiContentGenerated { timestamp, .. } => *timestamp,
            Self::OptimizationRequested { timestamp, .. } => *timestamp,
            Self::PatchProposed { timestamp, .. } => *timestamp,
            Self::PatchApplied { timestamp, .. } => *timestamp,
            Self::RolledBack { timestamp, .. } => *timestamp,
            Self::UserRated { timestamp, .. } => *timestamp,
            Self::QuizStarted { timestamp, .. } => *timestamp,
            Self::QuizCompleted { timestamp, .. } => *timestamp,
            Self::AnnotationRequested { timestamp, .. } => *timestamp,
            Self::AnnotationGenerated { timestamp, .. } => *timestamp,
            Self::FsrsUpdated { timestamp, .. } => *timestamp,
        }
    }

    /// 获取事件类型名称
    pub fn event_type(&self) -> &str {
        match self {
            Self::WordImported { .. } => "word_imported",
            Self::AiAnalysisRequested { .. } => "ai_analysis_requested",
            Self::AiContentGenerated { .. } => "ai_content_generated",
            Self::OptimizationRequested { .. } => "optimization_requested",
            Self::PatchProposed { .. } => "patch_proposed",
            Self::PatchApplied { .. } => "patch_applied",
            Self::RolledBack { .. } => "rolled_back",
            Self::UserRated { .. } => "user_rated",
            Self::QuizStarted { .. } => "quiz_started",
            Self::QuizCompleted { .. } => "quiz_completed",
            Self::AnnotationRequested { .. } => "annotation_requested",
            Self::AnnotationGenerated { .. } => "annotation_generated",
            Self::FsrsUpdated { .. } => "fsrs_updated",
        }
    }
}

/// AI 生成的内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiContent {
    pub etymology: Option<Etymology>,
    pub mnemonics: Vec<Mnemonic>,
    pub examples: Vec<PersonalizedExample>,
    pub scenes: Vec<Scene>,
}

/// 词源信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Etymology {
    pub origin: String,
    pub root_breakdown: Vec<Root>,
    pub historical_usage: Option<String>,
    pub cognates: Vec<String>,
}

/// 词根
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    pub part: String,    // "archi-"
    pub meaning: String, // "主要的"
    pub examples: Vec<String>,
}

/// 助记法
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mnemonic {
    pub mnemonic_type: MnemonicType,
    pub content: String,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MnemonicType {
    Etymology, // 词根词源
    Scene,     // 场景联想
    Homophone, // 谐音
    Visual,    // 视觉图像
    Chunking,  // 分块记忆
}

/// 个性化例句
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizedExample {
    pub text: String,
    pub context: String, // "技术场景" | "日常对话"
    pub difficulty: String,
    pub score: Option<f32>,
    pub user_feedback: Option<String>,
}

/// 场景记忆
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub description: String,
    pub image_prompt: Option<String>,
}

/// Patch 定义（AI 不直接改卡牌，而是提议 Patch）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardPatch {
    pub patch_id: String,
    pub target_field: String, // "mnemonic" | "examples" | "etymology"
    pub operation: PatchOperation,
    pub proposed_value: serde_json::Value,
    pub reasoning: String,
    pub confidence: f32,
    pub generated_by: String, // "gpt-4" | "claude-3"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchOperation {
    Replace,
    Append,
    Update { index: usize },
}

/// AI 批注
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub content: String,
    pub highlights: Vec<String>,
    pub trigger: String,
}

/// FSRS 评分
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Again, // 完全不记得
    Hard,  // 困难
    Good,  // 良好
    Easy,  // 简单
}

/// FSRS 卡片状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardState {
    pub stability: f64,
    pub difficulty: f64,
    pub elapsed_days: u32,
    pub scheduled_days: u32,
    pub reps: u32,
    pub lapses: u32,
    pub last_review: Option<i64>,
    pub next_review: i64,
}

impl Default for CardState {
    fn default() -> Self {
        Self {
            stability: 0.0,
            difficulty: 0.0,
            elapsed_days: 0,
            scheduled_days: 0,
            reps: 0,
            lapses: 0,
            last_review: None,
            next_review: chrono::Utc::now().timestamp(),
        }
    }
}

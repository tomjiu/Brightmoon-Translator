// Learning Plan Models
// 学习计划相关模型

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PlanType {
    Preset,   // 预设计划
    #[default]
    Custom,   // 自定义计划
    Imported, // 导入计划
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum TargetExam {
    Cet4,   // 大学英语四级
    Cet6,   // 大学英语六级
    Kaoyan, // 考研英语
    Ielts,  // 雅思
    Toefl,  // 托福
    Gre,    // GRE
    #[default]
    Custom, // 自定义
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PlanStatus {
    #[default]
    Active,    // 进行中
    Paused,    // 已暂停
    Completed, // 已完成
    Archived,  // 已归档
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningPlan {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub plan_type: PlanType,
    pub target_exam: TargetExam,
    pub total_words: i32,
    pub daily_target: i32,
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    pub status: PlanStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanWord {
    pub plan_id: String,
    pub word: String,
    pub word_order: i32,
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanProgress {
    pub plan_id: String,
    pub date: i64,
    pub words_learned: i32,
    pub words_reviewed: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetWordlist {
    pub id: String,
    pub name: String,
    pub name_zh: String,
    pub exam_type: TargetExam,
    pub word_count: i32,
    pub difficulty_level: i32,
    pub description: Option<String>,
    pub source_url: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSummary {
    pub plan: LearningPlan,
    pub progress: PlanProgressStats,
    pub today_target: i32,
    pub today_completed: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanProgressStats {
    pub total_words: i32,
    pub learned_words: i32,
    pub mastered_words: i32,
    pub remaining_words: i32,
    pub completion_rate: f64,
    pub days_elapsed: i32,
    pub estimated_days_remaining: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePlanRequest {
    pub name: String,
    pub description: Option<String>,
    pub plan_type: PlanType,
    pub target_exam: TargetExam,
    pub daily_target: i32,
    pub word_list: Vec<String>, // 词汇列表
}




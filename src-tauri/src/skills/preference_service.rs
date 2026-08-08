// preference_service.rs - 双偏好反馈闭环的聚合纯逻辑
use serde::{Deserialize, Serialize};

/// user_profile 表原始行
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferenceRow {
    pub card_id: String,
    pub field: String,
    pub rating: f64,
    pub feedback: Option<String>,
}

/// 聚合后的字段偏好
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldPreference {
    pub field: String,
    pub avg_rating: f64,
    pub rated_count: u32,
    pub last_feedback: Option<String>,
}

/// 观察偏好输入（来自 quiz_errors 聚合）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizPreferenceRow {
    pub quiz_type: String,
    pub correct: bool,
    pub count: u32,
}

/// 推断出的弱字段
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferredWeakField {
    pub field: String,
    pub strength: f64, // 错误率 0.0-1.0，越高越弱
}

/// 按 field 聚合 user_profile 行
pub fn aggregate_preferences(rows: &[UserPreferenceRow]) -> Vec<FieldPreference> {
    let mut grouped: std::collections::BTreeMap<String, (f64, u32, Option<String>)> =
        std::collections::BTreeMap::new();
    for row in rows {
        let entry = grouped
            .entry(row.field.clone())
            .or_insert((0.0, 0, None));
        entry.0 += row.rating;
        entry.1 += 1;
        if row.feedback.is_some() {
            entry.2 = row.feedback.clone();
        }
    }
    grouped
        .into_iter()
        .map(|(field, (sum, count, last_feedback))| FieldPreference {
            field,
            avg_rating: if count > 0 { sum / count as f64 } else { 0.0 },
            rated_count: count,
            last_feedback,
        })
        .collect()
}

/// 从测验结果推断弱字段（错误率 = 1 - correct 比例，超过阈值判弱）
pub fn infer_weak_fields(rows: &[QuizPreferenceRow], threshold: f64) -> Vec<InferredWeakField> {
    let mut grouped: std::collections::BTreeMap<String, (u32, u32)> =
        std::collections::BTreeMap::new();
    for row in rows {
        let entry = grouped.entry(row.quiz_type.clone()).or_insert((0, 0));
        if row.correct {
            entry.0 += row.count;
        } else {
            entry.1 += row.count;
        }
    }
    grouped
        .into_iter()
        .map(|(field, (correct, wrong))| {
            let total = correct + wrong;
            InferredWeakField {
                field,
                strength: if total > 0 { wrong as f64 / total as f64 } else { 0.0 },
            }
        })
        .filter(|f| f.strength >= threshold)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_preference_keeps_latest_rating() {
        let rows = vec![
            UserPreferenceRow {
                card_id: "c1".into(),
                field: "mnemonic".into(),
                rating: 2.0,
                feedback: Some("太复杂".into()),
            },
            UserPreferenceRow {
                card_id: "c1".into(),
                field: "mnemonic".into(),
                rating: 5.0,
                feedback: None,
            },
        ];
        let agg = aggregate_preferences(&rows);
        assert_eq!(agg.len(), 1);
        assert!((agg[0].avg_rating - 3.5).abs() < 0.01);
        assert_eq!(agg[0].rated_count, 2);
        assert!(agg[0].last_feedback.is_some());
    }

    #[test]
    fn infer_weak_fields_from_quiz_results() {
        let quiz_rows = vec![
            QuizPreferenceRow {
                quiz_type: "spelling".into(),
                correct: false,
                count: 3,
            },
            QuizPreferenceRow {
                quiz_type: "multiple_choice".into(),
                correct: true,
                count: 10,
            },
        ];
        let inferred = infer_weak_fields(&quiz_rows, 0.3);
        assert_eq!(inferred.len(), 1);
        assert_eq!(inferred[0].field, "spelling");
        assert!(inferred[0].strength >= 0.3);
    }
}

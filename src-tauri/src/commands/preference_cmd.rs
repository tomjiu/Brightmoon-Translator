// preference_cmd.rs - T12 双偏好反馈闭环命令层
use crate::skills::preference_service::{
    aggregate_preferences, infer_weak_fields, FieldPreference, InferredWeakField,
    QuizPreferenceRow, UserPreferenceRow,
};
use chrono::Utc;

/// 低分阈值判定：< 3 分触发再优化
pub fn needs_reoptimization(rating: f32) -> bool {
    rating < 3.0
}

/// 若传入 word 而非 card_id，查 cards 表解析
pub async fn resolve_card_id(pool: &sqlx::SqlitePool, word_or_id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT id FROM cards WHERE id = ?1 OR word = ?1")
        .bind(word_or_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// user_profile upsert（单测与命令共用）
pub async fn upsert_profile(
    pool: &sqlx::SqlitePool,
    card_id: &str,
    field: &str,
    rating: f64,
    feedback: Option<String>,
    now: i64,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO user_profile (card_id, field, rating, feedback, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT (card_id, field) DO UPDATE SET
            rating = excluded.rating,
            feedback = excluded.feedback,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(card_id)
    .bind(field)
    .bind(rating)
    .bind(&feedback)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 读取某组卡片的 user_profile 行
pub async fn load_preference_rows(
    pool: &sqlx::SqlitePool,
    card_ids: &[&str],
) -> Vec<UserPreferenceRow> {
    let mut rows = Vec::new();
    for id in card_ids {
        let r = sqlx::query_as::<_, (String, String, f64, Option<String>)>(
            "SELECT card_id, field, rating, feedback FROM user_profile WHERE card_id = ?",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        for (card_id, field, rating, feedback) in r {
            rows.push(UserPreferenceRow { card_id, field, rating, feedback });
        }
    }
    rows
}

/// 给字段打分（表达偏好）：写 user_profile + UserRated 事件
#[tauri::command]
pub async fn rate_card_field(
    state: tauri::State<'_, crate::AppState>,
    card_id: String,
    field: String,
    rating: f32,
    feedback: Option<String>,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();
    let now = Utc::now().timestamp();

    let card_id = resolve_card_id(pool, &card_id).await.ok_or("单词不存在")?;

    upsert_profile(pool, &card_id, &field, rating as f64, feedback.clone(), now).await?;

    let event = crate::domain::CardEvent::UserRated {
        field: field.clone(),
        score: rating,
        feedback,
        timestamp: now,
    };
    store
        .append_event(&card_id, &event)
        .await
        .map_err(|e| e.to_string())?;

    if needs_reoptimization(rating) {
        let ev = crate::domain::CardEvent::OptimizationRequested {
            field: field.clone(),
            reason: "user_feedback".to_string(),
            timestamp: now,
        };
        store.append_event(&card_id, &ev).await.map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 获取所有已评分字段的聚合偏好
#[tauri::command]
pub async fn get_user_preferences(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<FieldPreference>, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();
    let raw = sqlx::query_as::<_, (String, f64, Option<String>)>(
        "SELECT field, rating, feedback FROM user_profile",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let rows: Vec<UserPreferenceRow> = raw
        .into_iter()
        .map(|(field, rating, feedback)| UserPreferenceRow {
            card_id: String::new(),
            field,
            rating,
            feedback,
        })
        .collect();
    Ok(aggregate_preferences(&rows))
}

/// 从测验历史推断弱字段（观察偏好）
#[tauri::command]
pub async fn get_inferred_weak_fields(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<InferredWeakField>, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();
    let raw = sqlx::query_as::<_, (String, bool, i64)>(
        r#"
        SELECT quiz_type,
               (CASE WHEN user_answer = correct_answer THEN 1 ELSE 0 END) AS correct,
               COUNT(*) AS cnt
        FROM quiz_errors
        GROUP BY quiz_type, correct
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let rows: Vec<QuizPreferenceRow> = raw
        .into_iter()
        .map(|(quiz_type, correct, count)| QuizPreferenceRow {
            quiz_type,
            correct,
            count: count as u32,
        })
        .collect();
    Ok(infer_weak_fields(&rows, 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::preference_service::UserPreferenceRow;

    async fn in_memory_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect");
        sqlx::query(
            "CREATE TABLE user_profile (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id TEXT NOT NULL,
                field TEXT NOT NULL,
                rating REAL DEFAULT 0,
                feedback TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(card_id, field)
            )",
        )
        .execute(&pool)
        .await
        .expect("create");
        pool
    }

    #[tokio::test]
    async fn rate_field_upserts_profile() {
        let pool = in_memory_pool().await;
        let now = 1_700_000_000_000;
        upsert_profile(&pool, "c1", "mnemonic", 4.0, Some("不错".into()), now)
            .await
            .expect("upsert");
        upsert_profile(&pool, "c1", "mnemonic", 2.0, None, now + 1)
            .await
            .expect("upsert");
        let rows: Vec<UserPreferenceRow> = load_preference_rows(&pool, &["c1"]).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].field, "mnemonic");
        assert!((rows[0].rating - 2.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn low_rating_flags_for_optimization() {
        assert!(needs_reoptimization(2.0));
        assert!(!needs_reoptimization(4.0));
    }

    #[tokio::test]
    async fn rate_by_word_resolves_card_id() {
        let pool = in_memory_pool().await;
        sqlx::query("CREATE TABLE cards (id TEXT PRIMARY KEY, word TEXT, fsrs_state TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO cards (id, word, fsrs_state) VALUES ('c1', 'hello', '{}')")
            .execute(&pool)
            .await
            .unwrap();

        let resolved = resolve_card_id(&pool, "hello").await;
        assert_eq!(resolved.as_deref(), Some("c1"));
    }
}

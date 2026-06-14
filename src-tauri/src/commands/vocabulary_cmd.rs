// Vocabulary Commands - 词汇学习 API

use crate::domain::{AiContent, LearningPhase, LearningState, Rating, WordCard};
use crate::infrastructure::EventStore;
use crate::skills::{GenerateCardSkill, OpenAiCompatibleProvider, SkillInput, SkillRegistry};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tauri::State;

/// 应用状态
pub struct AppState {
    pub pool: SqlitePool,
    pub event_store: EventStore,
    pub skill_registry: Arc<tokio::sync::RwLock<SkillRegistry>>,
}

/// 卡牌信息（简化版，用于列表）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInfo {
    pub id: String,
    pub word: String,
    pub phase: LearningPhase,
    pub next_review: i64,
    pub reps: u32,
    pub stability: f64,
}

/// 核心词库词条
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreVocabEntry {
    pub word: String,
    pub frequency_rank: i64,
    pub frq: Option<i64>,
    pub collins: Option<i64>,
    pub oxford: Option<i64>,
    pub tag: Option<String>,
}

/// 获取核心词库列表
#[tauri::command]
pub async fn get_core_vocabulary(
    state: State<'_, AppState>,
    offset: i64,
    limit: i64,
) -> Result<Vec<CoreVocabEntry>, String> {
    let rows = sqlx::query(
        r#"
        SELECT word, frequency_rank, frq, collins, oxford, tag
        FROM core_vocabulary
        ORDER BY frequency_rank
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let entries = rows
        .into_iter()
        .map(|row| CoreVocabEntry {
            word: row.try_get("word").unwrap(),
            frequency_rank: row.try_get("frequency_rank").unwrap(),
            frq: row.try_get("frq").ok(),
            collins: row.try_get("collins").ok(),
            oxford: row.try_get("oxford").ok(),
            tag: row.try_get("tag").ok(),
        })
        .collect();

    Ok(entries)
}

/// 搜索核心词库
#[tauri::command]
pub async fn search_core_vocabulary(
    state: State<'_, AppState>,
    query: String,
    limit: i64,
) -> Result<Vec<CoreVocabEntry>, String> {
    let pattern = format!("%{}%", query);

    let rows = sqlx::query(
        r#"
        SELECT word, frequency_rank, frq, collins, oxford, tag
        FROM core_vocabulary
        WHERE word LIKE ?
        ORDER BY frequency_rank
        LIMIT ?
        "#,
    )
    .bind(&pattern)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let entries = rows
        .into_iter()
        .map(|row| CoreVocabEntry {
            word: row.try_get("word").unwrap(),
            frequency_rank: row.try_get("frequency_rank").unwrap(),
            frq: row.try_get("frq").ok(),
            collins: row.try_get("collins").ok(),
            oxford: row.try_get("oxford").ok(),
            tag: row.try_get("tag").ok(),
        })
        .collect();

    Ok(entries)
}

/// 创建新卡牌
#[tauri::command]
pub async fn create_card(state: State<'_, AppState>, word: String) -> Result<String, String> {
    use crate::domain::CardEvent;
    use chrono::Utc;
    use uuid::Uuid;

    let card_id = Uuid::new_v4().to_string();

    let event = CardEvent::WordImported {
        word: word.clone(),
        source: "manual".to_string(),
        timestamp: Utc::now().timestamp(),
    };

    state
        .event_store
        .append_event(&card_id, &event)
        .await
        .map_err(|e| e.to_string())?;

    Ok(card_id)
}

/// 获取卡牌详情
#[tauri::command]
pub async fn get_card(state: State<'_, AppState>, card_id: String) -> Result<WordCard, String> {
    state
        .event_store
        .rebuild_card(&card_id)
        .await
        .map_err(|e| e.to_string())
}

/// 获取待复习卡牌列表
#[tauri::command]
pub async fn get_due_cards(state: State<'_, AppState>) -> Result<Vec<CardInfo>, String> {
    use chrono::Utc;

    let now = Utc::now().timestamp();

    let rows = sqlx::query(
        r#"
        SELECT id, word, fsrs_state, learning_state
        FROM cards
        WHERE json_extract(fsrs_state, '$.next_review') <= ?
        ORDER BY json_extract(fsrs_state, '$.next_review')
        LIMIT 100
        "#,
    )
    .bind(now)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut cards = Vec::new();
    for row in rows {
        let id: String = row.try_get("id").unwrap();
        let word: String = row.try_get("word").unwrap();
        let fsrs_json: String = row.try_get("fsrs_state").unwrap();
        let learning_json: Option<String> = row.try_get("learning_state").ok();

        let fsrs_state: crate::domain::CardState =
            serde_json::from_str(&fsrs_json).unwrap_or_default();
        let learning_state: LearningState = learning_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        cards.push(CardInfo {
            id,
            word,
            phase: learning_state.phase,
            next_review: fsrs_state.next_review,
            reps: fsrs_state.reps,
            stability: fsrs_state.stability,
        });
    }

    Ok(cards)
}

/// AI 生成卡牌内容
#[tauri::command]
pub async fn generate_card_content(
    state: State<'_, AppState>,
    card_id: String,
) -> Result<AiContent, String> {
    let card = state
        .event_store
        .rebuild_card(&card_id)
        .await
        .map_err(|e| e.to_string())?;

    let registry = state.skill_registry.read().await;

    let context = serde_json::json!({
        "word": card.word,
        "definition": card.base_data.definitions.first().map(|d| d.as_str()),
        "translation": card.base_data.translation,
    });

    let input = SkillInput::new(&card.word).with_param("context", context);

    let output = registry
        .execute("generate_card", input)
        .await
        .map_err(|e| e.to_string())?;

    let model = output
        .metadata
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let ai_content: AiContent = output.into_type().map_err(|e| e.to_string())?;

    // 记录事件
    use crate::domain::CardEvent;
    use chrono::Utc;

    let event = CardEvent::AiContentGenerated {
        content: ai_content.clone(),
        model,
        confidence: 0.9,
        timestamp: Utc::now().timestamp(),
    };

    state
        .event_store
        .append_event(&card_id, &event)
        .await
        .map_err(|e| e.to_string())?;

    Ok(ai_content)
}

/// 提交复习结果
#[tauri::command]
pub async fn submit_review(
    state: State<'_, AppState>,
    card_id: String,
    rating: Rating,
) -> Result<(), String> {
    use crate::domain::{CardEvent, FsrsEngine};
    use chrono::Utc;

    let card = state
        .event_store
        .rebuild_card(&card_id)
        .await
        .map_err(|e| e.to_string())?;

    // 计算新的 FSRS 状态
    let fsrs = FsrsEngine::new();
    let new_state = fsrs
        .schedule_review(&card.fsrs_state, rating, Utc::now())
        .map_err(|e| e.to_string())?;

    // 记录事件
    let event = CardEvent::FsrsUpdated {
        grade: rating,
        new_state,
        timestamp: Utc::now().timestamp(),
    };

    state
        .event_store
        .append_event(&card_id, &event)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 获取学习统计
#[tauri::command]
pub async fn get_learning_stats(state: State<'_, AppState>) -> Result<LearningStats, String> {
    let total_cards: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cards")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let due_cards: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM cards
        WHERE json_extract(fsrs_state, '$.next_review') <= ?
        "#,
    )
    .bind(chrono::Utc::now().timestamp())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(LearningStats {
        total_cards: total_cards as u32,
        due_cards: due_cards as u32,
        learned_today: 0,  // TODO: 实现
        reviewed_today: 0, // TODO: 实现
    })
}

/// 学习统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStats {
    pub total_cards: u32,
    pub due_cards: u32,
    pub learned_today: u32,
    pub reviewed_today: u32,
}

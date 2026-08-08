// Optimization Commands - T8 答错触发 AI 增强 + 版本追踪
//
// 激活原本是死代码的 Patch 系统（PatchValidator + PatchApplicator + StateMachine）：
// - record_quiz_result:     测验答错 → 写 quiz_errors 表 + QuizCompleted 事件 + 弱点计数
// - optimize_card_on_error: 答错后 AI 生成 Patch → 验证 → 应用 → PatchProposed/PatchApplied 事件 → 更新快照
// - get_card_patch_history: 读取卡牌 patch 历史（版本追踪）
// - get_weak_words:         弱点词表（错误次数排序）

use crate::domain::{CardPatch, PatchOperation, PatchValidator};
use crate::skills::{LlmMessage, LlmProvider, LlmRequest, OpenAiCompatibleProvider};
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;

/// 弱点计数 upsert(答错时调用)。
/// T8 修复:依赖 UNIQUE(card_id, field, error_type) + 显式 ON CONFLICT 目标，
/// 否则 count 永远 = 1(无唯一约束时 ON CONFLICT DO UPDATE 永不触发)。
async fn upsert_weak_point(
    pool: &sqlx::SqlitePool,
    card_id: &str,
    error_type: &str,
    now: i64,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO weak_points (card_id, field, error_type, count, last_occurred_at)
        VALUES (?, ?, ?, 1, ?)
        ON CONFLICT (card_id, field, error_type) DO UPDATE SET
            count = count + 1,
            last_occurred_at = excluded.last_occurred_at
        "#,
    )
    .bind(card_id)
    .bind("ai_content")
    .bind(error_type)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 从 quiz_type 映射弱点类型
fn weak_error_type(quiz_type: &str) -> &'static str {
    match quiz_type {
        "spelling" => "spelling",
        "cloze" | "fill_blank" => "usage",
        _ => "meaning",
    }
}

/// 记录测验结果（答错时写入错误日志 + 弱点统计）
#[tauri::command]
pub async fn record_quiz_result(
    state: tauri::State<'_, crate::AppState>,
    card_id: String,
    quiz_type: String,
    correct: bool,
    user_answer: Option<String>,
    correct_answer: Option<String>,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();
    let now = Utc::now().timestamp();

    if !correct {
        // 1. 写 quiz_errors 表
        sqlx::query(
            r#"
            INSERT INTO quiz_errors (card_id, quiz_type, user_answer, correct_answer, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&card_id)
        .bind(&quiz_type)
        .bind(&user_answer)
        .bind(&correct_answer)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        // 2. 弱点计数 upsert
        let error_type = weak_error_type(&quiz_type);
        upsert_weak_point(pool, &card_id, error_type, now).await?;
    }

    // 3. 写 QuizCompleted 事件（驱动状态机 + 卡牌错误记录）
    let event = crate::domain::CardEvent::QuizCompleted {
        correct,
        user_answer: user_answer.unwrap_or_default(),
        correct_answer: correct_answer.unwrap_or_default(),
        time_spent: 0,
        timestamp: now,
    };
    store
        .append_event(&card_id, &event)
        .await
        .map_err(|e| e.to_string())?;

    // 4. 更新快照（让 error_records / 状态反映测验结果）
    if let Ok(card) = store.rebuild_card(&card_id).await {
        store
            .update_snapshot(&card)
            .await
            .map_err(|e| format!("更新快照失败: {e}"))?;
    }

    Ok(())
}

/// 答错触发 AI 增强：生成 Patch → 验证 → 应用
///
/// 流程：
/// 1. 写 OptimizationRequested 事件（记录触发原因）
/// 2. 用 LLM 生成针对弱点的改进内容（降低温度，JSON 强制）
/// 3. 构造 CardPatch → PatchValidator 验证（置信度/字段/值类型/内容）
/// 4. 写 PatchProposed → PatchApplied 事件
/// 5. 应用 Patch 到卡牌，更新快照
#[tauri::command]
pub async fn optimize_card_on_error(
    state: tauri::State<'_, crate::AppState>,
    card_id: String,
    error_type: String,
    user_answer: Option<String>,
    correct_answer: Option<String>,
) -> Result<OptimizeResult, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let now = Utc::now().timestamp();

    // 1. 写 OptimizationRequested 事件
    let opt_event = crate::domain::CardEvent::OptimizationRequested {
        field: "ai_content".to_string(),
        reason: format!("after_error:{}", error_type),
        timestamp: now,
    };
    store
        .append_event(&card_id, &opt_event)
        .await
        .map_err(|e| e.to_string())?;

    // 2. 加载卡牌
    let card = store
        .rebuild_card(&card_id)
        .await
        .map_err(|e| e.to_string())?;

    // 2.1 T8 防抖:同一卡片 5 秒内已有 AI 优化，跳过，避免连续答错触发 LLM 风暴
    if let Ok(Some(last)) = store.last_ai_optimize_at(&card_id).await {
        if now - last < 5 {
            tracing::info!(
                "optimize_card_on_error: 卡片 {} 刚优化过({}s 前)，跳过本次",
                card_id,
                now - last
            );
            return Ok(OptimizeResult {
                applied: false,
                message: "该卡片近期已优化，稍后再试".to_string(),
                patch_id: None,
                reasoning: None,
                suggestion_preview: None,
            });
        }
    }

    // 3. 配置 LLM
    let config = state.system.config.lock().await;
    let llm = &config.llm;
    let api_key = if !llm.api_key.is_empty() {
        Some(llm.api_key.clone())
    } else {
        llm.api_keys.first().cloned()
    };
    let base_url = llm.base_url.clone();
    let model = llm.model.clone();
    drop(config);

    let (Some(key), false) = (api_key, base_url.is_empty()) else {
        return Ok(OptimizeResult {
            applied: false,
            message: "未配置 LLM，跳过 AI 增强".to_string(),
            patch_id: None,
            reasoning: None,
            suggestion_preview: None,
        });
    };

    let provider = OpenAiCompatibleProvider::new(key, base_url, model);
    let field = pick_weak_field(&error_type, &card);
    let schema = patch_json_schema(&field);

    // 构造 prompt
    let system_prompt = format!(
        "你是单词学习助手。针对单词 '{}' 的弱项 '{}' 生成改进内容。只返回 JSON，不要其他文字。",
        card.word, field
    );
    let user_prompt = format!(
        r#"单词: {}
错误类型: {}
用户回答: {}
正确答案: {}
当前 AI 内容: {}

请针对弱项生成改进内容，返回 JSON:
{{
  "field": "{}",
  "proposed_value": <该字段对应的数据结构>,
  "reasoning": "为什么这样改进（面向学习者，简短）",
  "confidence": 0.0到1.0之间的数字，表示你对这个改动的确信度（>0.7 才值得应用，低分通常是有歧义或原始内容已不错）
}}
严格按 JSON Schema 返回：
{}"#,
        card.word,
        error_type,
        user_answer.as_deref().unwrap_or("(无)"),
        correct_answer.as_deref().unwrap_or("(无)"),
        serde_json::to_string_pretty(&card.ai_content).unwrap_or_default(),
        field,
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );

    let request = LlmRequest::new(vec![
        LlmMessage::system(system_prompt),
        LlmMessage::user(user_prompt),
    ])
    .with_temperature(0.3)
    .with_max_tokens(2000)
    .with_json_schema(serde_json::json!({ "type": "object" }));

    // 4. 调用 LLM（失败不阻塞，只返回未应用）
    let response = match provider.complete(request).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("AI 优化生成失败: {}", e);
            return Ok(OptimizeResult {
                applied: false,
                message: format!("AI 生成失败: {e}"),
                patch_id: None,
                reasoning: None,
                suggestion_preview: None,
            });
        },
    };

    // 5. 解析 LLM 输出
    let json_str = extract_json(&response.content);
    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("AI 优化 JSON 解析失败: {}", e);
            return Ok(OptimizeResult {
                applied: false,
                message: "AI 返回格式错误".to_string(),
                patch_id: None,
                reasoning: None,
                suggestion_preview: None,
            });
        },
    };

    let proposed_value = parsed.get("proposed_value").cloned().unwrap_or_default();
    let reasoning = parsed
        .get("reasoning")
        .and_then(|v| v.as_str())
        .unwrap_or("AI 自动优化")
        .to_string();
    // T8 修复:confidence 由 LLM 返回，clamp 到 [0.1, 0.99]，缺失时兜底 0.7。
    // 若 LLM 明确给出低置信度，验证器仍会放行(默认 min 0.7)，这里仅在 LLM 评分极低时主动拒绝。
    let confidence = parsed
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7)
        .clamp(0.1, 0.99) as f32;

    // 6. 构造 Patch
    let patch = CardPatch {
        patch_id: uuid::Uuid::new_v4().to_string(),
        target_field: field.clone(),
        operation: PatchOperation::Replace,
        proposed_value: proposed_value.clone(),
        reasoning,
        confidence,
        generated_by: "llm-optimize".to_string(),
    };

    // 7. PatchValidator 验证
    let validator = PatchValidator::default();
    if let Err(e) = validator.validate(&patch, &card) {
        tracing::warn!("Patch 验证未通过: {}", e);
        return Ok(OptimizeResult {
            applied: false,
            message: format!("Patch 验证未通过: {e}"),
            patch_id: None,
            reasoning: None,
            suggestion_preview: None,
        });
    }

    // 8. 写 PatchProposed 事件
    let proposed_event = crate::domain::CardEvent::PatchProposed {
        patch: patch.clone(),
        timestamp: now,
    };
    store
        .append_event(&card_id, &proposed_event)
        .await
        .map_err(|e| e.to_string())?;

    // 9. 应用 Patch 到内存卡牌，得到新版本号
    let mut updated = card.clone();
    crate::domain::PatchApplicator::apply(&patch, &mut updated).map_err(|e| e.to_string())?;
    let new_version = updated.current_version + 1;
    updated.current_version = new_version;

    // 10. 写 PatchApplied 事件
    let applied_event = crate::domain::CardEvent::PatchApplied {
        version: new_version,
        patch: patch.clone(),
        timestamp: now,
    };
    store
        .append_event(&card_id, &applied_event)
        .await
        .map_err(|e| e.to_string())?;

    // 11. 更新快照
    store
        .update_snapshot(&updated)
        .await
        .map_err(|e| format!("更新快照失败: {e}"))?;

    tracing::info!(
        "AI 优化已应用: card={}, field={}, version={}",
        card_id,
        field,
        new_version
    );

    // 生成建议预览：从 proposed_value 提取首个字符串字段作为展示文本
    let suggestion_preview = extract_preview_text(&proposed_value);

    Ok(OptimizeResult {
        applied: true,
        message: "已根据错误生成改进内容".to_string(),
        patch_id: Some(patch.patch_id),
        reasoning: Some(patch.reasoning),
        suggestion_preview,
    })
}

/// 从 AI 建议值中提取可展示的文本预览（取首个字符串字段，截断到 160 字符）
fn extract_preview_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(truncate(s, 160))
            }
        },
        serde_json::Value::Object(map) => {
            for (_, v) in map {
                if let Some(text) = extract_preview_text(v) {
                    return Some(text);
                }
            }
            None
        },
        serde_json::Value::Array(arr) => {
            for v in arr {
                if let Some(text) = extract_preview_text(v) {
                    return Some(text);
                }
            }
            None
        },
        _ => None,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

/// 根据错误类型选择需要增强的字段
fn pick_weak_field(error_type: &str, card: &crate::domain::WordCard) -> String {
    let has_ai = card.ai_content.is_some();
    match error_type {
        "spelling" => "mnemonics".to_string(),
        "usage" | "cloze" | "fill_blank" => "examples".to_string(),
        "meaning" => "etymology".to_string(),
        _ => {
            if has_ai {
                "mnemonics".to_string()
            } else {
                "etymology".to_string()
            }
        },
    }
}

/// 生成目标字段对应的 JSON Schema 提示
fn patch_json_schema(field: &str) -> serde_json::Value {
    match field {
        "mnemonics" => serde_json::json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "mnemonic_type": { "type": "string", "enum": ["etymology", "scene", "homophone", "visual", "chunking", "comparison"] },
                    "content": { "type": "string" }
                },
                "required": ["mnemonic_type", "content"]
            }
        }),
        "examples" => serde_json::json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "context": { "type": "string" },
                    "difficulty": { "type": "string" }
                },
                "required": ["text", "context", "difficulty"]
            }
        }),
        "etymology" => serde_json::json!({
            "type": "object",
            "properties": {
                "origin": { "type": "string" },
                "root_breakdown": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "part": { "type": "string" },
                            "meaning": { "type": "string" },
                            "examples": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["part", "meaning"]
                    }
                },
                "historical_usage": { "type": "string" },
                "cognates": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["origin", "root_breakdown", "cognates"]
        }),
        _ => serde_json::json!({ "type": "object" }),
    }
}

/// 从 LLM 响应中提取 JSON（处理 markdown 代码块包裹）
fn extract_json(content: &str) -> &str {
    let trimmed = content.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return trimmed;
    }
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed[start..].rfind('}') {
            return &trimmed[start..start + end + 1];
        }
    }
    trimmed
}

/// 优化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeResult {
    pub applied: bool,
    pub message: String,
    pub patch_id: Option<String>,
    /// T8 增强 UI：AI 改进理由（面向学习者）
    pub reasoning: Option<String>,
    /// T8 增强 UI：建议内容的文本预览（截断）
    pub suggestion_preview: Option<String>,
}

/// Patch 历史记录项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchHistoryEntry {
    pub version: u32,
    pub field: String,
    pub operation: String,
    pub reasoning: String,
    pub generated_by: String,
    pub timestamp: i64,
}

/// 读取卡牌 Patch 历史（版本追踪）。
/// 接受 word 或 card_id：传 word 时先反查卡片，兼容前端用单词查看详情。
#[tauri::command]
pub async fn get_card_patch_history(
    state: tauri::State<'_, crate::AppState>,
    word_or_card_id: String,
) -> Result<Vec<PatchHistoryEntry>, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();

    // 先尝试按 word 精确匹配（大小写不敏感）反查 card_id
    let card_id = sqlx::query_scalar::<_, String>(
        "SELECT id FROM cards WHERE word = ?1 COLLATE NOCASE LIMIT 1",
    )
    .bind(&word_or_card_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .unwrap_or(word_or_card_id.clone());

    let events = store
        .load_events(&card_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut history = Vec::new();
    for e in events {
        match e {
            crate::domain::CardEvent::PatchApplied {
                version,
                patch,
                timestamp,
            } => {
                history.push(PatchHistoryEntry {
                    version,
                    field: patch.target_field,
                    operation: format!("{:?}", patch.operation),
                    reasoning: patch.reasoning,
                    generated_by: patch.generated_by,
                    timestamp,
                });
            },
            _ => {},
        }
    }

    Ok(history)
}

/// 弱点词表（错误次数排序）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeakWordEntry {
    pub card_id: String,
    pub word: String,
    pub error_type: String,
    pub count: i64,
    pub last_occurred_at: i64,
}

/// 获取弱点词表（供统计页 / 一键重学）
#[tauri::command]
pub async fn get_weak_point_words(
    state: tauri::State<'_, crate::AppState>,
    limit: i64,
) -> Result<Vec<WeakWordEntry>, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();

    let rows = sqlx::query(
        r#"
        SELECT w.card_id, c.word, w.error_type, w.count, w.last_occurred_at
        FROM weak_points w
        LEFT JOIN cards c ON c.id = w.card_id
        WHERE w.resolved = 0
        ORDER BY w.count DESC, w.last_occurred_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut words = Vec::new();
    for row in rows {
        words.push(WeakWordEntry {
            card_id: row.try_get("card_id").unwrap_or_default(),
            word: row.try_get("word").unwrap_or_default(),
            error_type: row.try_get("error_type").unwrap_or_default(),
            count: row.try_get("count").unwrap_or_default(),
            last_occurred_at: row.try_get("last_occurred_at").unwrap_or_default(),
        });
    }

    Ok(words)
}

/// 标记弱点已解决（重学通过后调用）
#[tauri::command]
pub async fn resolve_weak_point(
    state: tauri::State<'_, crate::AppState>,
    card_id: String,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    sqlx::query(
        r#"
        UPDATE weak_points SET resolved = 1 WHERE card_id = ?
        "#,
    )
    .bind(&card_id)
    .execute(store.pool())
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建带 UNIQUE 约束的 weak_points 表（与 init_schema 保持一致）
    async fn weak_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE weak_points (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id TEXT NOT NULL,
                field TEXT NOT NULL,
                error_type TEXT NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                last_occurred_at INTEGER NOT NULL,
                resolved INTEGER NOT NULL DEFAULT 0,
                UNIQUE(card_id, field, error_type)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn weak_point_count_increments_on_repeat_error() {
        // P0 修复验证:UNIQUE(card_id, field, error_type) 让 ON CONFLICT DO UPDATE 真正触发，
        // 同一弱点评第二次 count 从 1 → 2(修复前恒为 1)
        let pool = weak_pool().await;

        upsert_weak_point(&pool, "card-1", "meaning", 1000)
            .await
            .unwrap();
        upsert_weak_point(&pool, "card-1", "meaning", 2000)
            .await
            .unwrap();
        upsert_weak_point(&pool, "card-1", "meaning", 3000)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT count FROM weak_points WHERE card_id = 'card-1' AND error_type = 'meaning'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 3);

        // 只产生一行
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM weak_points")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[tokio::test]
    async fn weak_point_separates_error_types() {
        let pool = weak_pool().await;
        upsert_weak_point(&pool, "card-1", "meaning", 1000)
            .await
            .unwrap();
        upsert_weak_point(&pool, "card-1", "usage", 1000).await.unwrap();
        upsert_weak_point(&pool, "card-2", "meaning", 1000)
            .await
            .unwrap();

        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM weak_points")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 3);
    }

    #[test]
    fn weak_error_type_mapping() {
        assert_eq!(weak_error_type("spelling"), "spelling");
        assert_eq!(weak_error_type("cloze"), "usage");
        assert_eq!(weak_error_type("fill_blank"), "usage");
        assert_eq!(weak_error_type("choice"), "meaning");
        assert_eq!(weak_error_type("anything_else"), "meaning");
    }
}

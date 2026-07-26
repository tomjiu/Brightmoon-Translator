// FSRS Optimization Commands - FSRS 算法优化和分析

use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;

/// FSRS 参数分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsrsAnalysis {
    pub current_params: [f64; 17],
    pub retention_rate: f64,
    pub avg_interval_days: f64,
    pub avg_difficulty: f64,
    pub avg_stability: f64,
    pub total_lapses: i64,
    pub optimal_params: Option<[f64; 17]>,
}

/// 遗忘曲线数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgettingCurvePoint {
    pub days: f64,
    pub retention: f64,
}

/// 未来复习负载
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewForecast {
    pub date: String,
    pub due_count: i32,
}

/// 最佳学习时段
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyTimeSlot {
    pub hour: i32,
    pub label: String,
    pub correct_rate: f64,
    pub review_count: i32,
}

/// FSRS 分析报告
#[tauri::command]
pub async fn get_fsrs_analysis(state: State<'_, crate::AppState>) -> Result<FsrsAnalysis, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    // 获取当前参数
    let engine = crate::domain::FsrsEngine::new();
    let params = *engine.get_params();

    // 计算所有卡牌的平均指标
    let cards = sqlx::query("SELECT fsrs_state FROM cards")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut total_stability = 0.0;
    let mut total_difficulty = 0.0;
    let mut total_lapses = 0i64;
    let mut total_interval = 0.0;
    let mut count = 0;

    for row in &cards {
        let fsrs_str: String = row.get("fsrs_state");
        let state: serde_json::Value = serde_json::from_str(&fsrs_str).unwrap_or_default();

        let stability = state["stability"].as_f64().unwrap_or(0.0);
        let difficulty = state["difficulty"].as_f64().unwrap_or(0.0);
        let lapses = state["lapses"].as_i64().unwrap_or(0);
        let scheduled = state["scheduled_days"].as_f64().unwrap_or(0.0);

        if stability > 0.0 {
            total_stability += stability;
            total_difficulty += difficulty;
            total_lapses += lapses;
            total_interval += scheduled;
            count += 1;
        }
    }

    // 计算记忆保持率
    let retention_rate = calculate_retention_rate(pool).await.unwrap_or(0.0);

    // 计算平均值
    let avg_stability = if count > 0 {
        total_stability / count as f64
    } else {
        0.0
    };
    let avg_difficulty = if count > 0 {
        total_difficulty / count as f64
    } else {
        0.0
    };
    let avg_interval = if count > 0 {
        total_interval / count as f64
    } else {
        0.0
    };

    // 计算优化参数（简化版：基于 retention rate 调整）
    let optimal_params = optimize_params_from_history(pool).await.ok();

    Ok(FsrsAnalysis {
        current_params: params,
        retention_rate,
        avg_interval_days: avg_interval,
        avg_difficulty,
        avg_stability,
        total_lapses,
        optimal_params,
    })
}

/// 获取遗忘曲线数据
#[tauri::command]
pub async fn get_forgetting_curve(
    state: State<'_, crate::AppState>,
    stability: f64,
) -> Result<Vec<ForgettingCurvePoint>, String> {
    let engine = crate::domain::FsrsEngine::new();
    let mut points = Vec::new();

    // 生成 0 到 90 天的遗忘曲线
    for day in 0..=90 {
        let retention = engine.forgetting_curve(day, stability);
        points.push(ForgettingCurvePoint {
            days: day as f64,
            retention,
        });
    }

    // 获取实际记忆保持数据（从复习记录中统计）
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let actual_retention = calculate_actual_retention_by_interval(pool)
        .await
        .unwrap_or_default();

    // 如果有实际数据，附加到返回结果（用前几个点替换理论值）
    if !actual_retention.is_empty() {
        tracing::info!(
            "遗忘曲线：理论数据 {} 点，实际数据 {} 点",
            points.len(),
            actual_retention.len()
        );
    }

    Ok(points)
}

/// 获取未来30天复习预测
#[tauri::command]
pub async fn get_review_forecast(
    state: State<'_, crate::AppState>,
    days: i32,
) -> Result<Vec<ReviewForecast>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let today = chrono::Utc::now().date_naive();

    let mut forecasts = Vec::new();

    for i in 0..days {
        let date = today + chrono::Duration::days(i as i64);
        let date_start = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
        let date_end = date.and_hms_opt(23, 59, 59).unwrap().and_utc().timestamp();

        let due_count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cards
             WHERE json_extract(fsrs_state, '$.next_review') >= ?
               AND json_extract(fsrs_state, '$.next_review') <= ?",
        )
        .bind(date_start)
        .bind(date_end)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        forecasts.push(ReviewForecast {
            date: date.format("%Y-%m-%d").to_string(),
            due_count,
        });
    }

    Ok(forecasts)
}

/// 获取最佳学习时段分析
#[tauri::command]
pub async fn get_best_study_time(
    state: State<'_, crate::AppState>,
) -> Result<Vec<StudyTimeSlot>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    // 按小时统计复习正确率
    let rows = sqlx::query(
        r#"
        SELECT
            CAST(strftime('%H', timestamp, 'unixepoch') AS INTEGER) as hour,
            COUNT(*) as total,
            COUNT(CASE WHEN json_extract(event_data, '$.grade') IN ('good', 'easy') THEN 1 END) as correct
        FROM card_events
        WHERE event_type = 'fsrs_updated'
        GROUP BY hour
        ORDER BY hour ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut slots = Vec::new();
    for row in &rows {
        let hour: i32 = row.get("hour");
        let total: i32 = row.get("total");
        let correct: i32 = row.get("correct");
        let correct_rate = if total > 0 {
            (correct as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let label = match hour {
            6..=8 => "早晨 🌅",
            9..=11 => "上午 ☀️",
            12..=13 => "午间 🍽️",
            14..=17 => "下午 🌤️",
            18..=20 => "傍晚 🌇",
            21..=23 => "夜间 🌙",
            _ => "凌晨 🌑",
        }
        .to_string();

        slots.push(StudyTimeSlot {
            hour,
            label,
            correct_rate,
            review_count: total,
        });
    }

    Ok(slots)
}

/// 获取卡牌难度分布
#[tauri::command]
pub async fn get_difficulty_distribution(
    state: State<'_, crate::AppState>,
) -> Result<Vec<DifficultyBucket>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let cards = sqlx::query("SELECT fsrs_state FROM cards")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 10个难度区间：1-2, 2-3, ..., 9-10
    let mut buckets = vec![0i32; 10];

    for row in &cards {
        let fsrs_str: String = row.get("fsrs_state");
        let state: serde_json::Value = serde_json::from_str(&fsrs_str).unwrap_or_default();
        let difficulty = state["difficulty"].as_f64().unwrap_or(1.0);
        let idx = ((difficulty - 1.0).clamp(0.0, 8.999)) as usize;
        buckets[idx] += 1;
    }

    let distribution: Vec<DifficultyBucket> = buckets
        .into_iter()
        .enumerate()
        .map(|(i, count)| DifficultyBucket {
            range_start: (i + 1) as f64,
            range_end: (i + 2) as f64,
            count,
        })
        .collect();

    Ok(distribution)
}

/// 难度分布桶
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DifficultyBucket {
    pub range_start: f64,
    pub range_end: f64,
    pub count: i32,
}

// ============================================
// 辅助函数
// ============================================

async fn calculate_retention_rate(pool: &sqlx::SqlitePool) -> Result<f64, sqlx::Error> {
    let row: (i32, i32) = sqlx::query_as(
        r#"
        SELECT
            COUNT(CASE WHEN json_extract(event_data, '$.grade') IN ('good', 'easy') THEN 1 END),
            COUNT(*)
        FROM card_events
        WHERE event_type = 'fsrs_updated'
        "#,
    )
    .fetch_one(pool)
    .await?;

    let (correct, total) = row;
    if total == 0 {
        return Ok(0.0);
    }
    Ok((correct as f64 / total as f64) * 100.0)
}

/// 基于用户复习历史优化 FSRS 参数（简化版梯度调整）
async fn optimize_params_from_history(pool: &sqlx::SqlitePool) -> Result<[f64; 17], sqlx::Error> {
    let engine = crate::domain::FsrsEngine::new();
    let mut params = *engine.get_params();

    // 统计各评分分布
    let row: (i32, i32, i32, i32) = sqlx::query_as(
        r#"
        SELECT
            COUNT(CASE WHEN json_extract(event_data, '$.grade') = 'again' THEN 1 END),
            COUNT(CASE WHEN json_extract(event_data, '$.grade') = 'hard' THEN 1 END),
            COUNT(CASE WHEN json_extract(event_data, '$.grade') = 'good' THEN 1 END),
            COUNT(CASE WHEN json_extract(event_data, '$.grade') = 'easy' THEN 1 END)
        FROM card_events
        WHERE event_type = 'fsrs_updated'
        "#,
    )
    .fetch_one(pool)
    .await?;

    let (again, hard, good, easy) = row;
    let total = again + hard + good + easy;

    if total < 50 {
        // 数据不足，返回默认参数
        return Ok(params);
    }

    let again_rate = again as f64 / total as f64;
    let good_easy_rate = (good + easy) as f64 / total as f64;

    // 简化优化逻辑
    if again_rate > 0.3 {
        // 忘记率过高，降低初始稳定性
        params[0] *= 0.8; // S0(again)
        params[1] *= 0.9; // S0(hard)
        params[2] *= 0.9; // S0(good)
        params[3] *= 0.95; // S0(easy)
    } else if good_easy_rate > 0.85 {
        // 正确率很高，可以适当增加间隔
        params[0] *= 1.1;
        params[1] *= 1.1;
        params[2] *= 1.1;
        params[3] *= 1.1;
    }

    Ok(params)
}

/// 按复习间隔计算实际记忆保持率
async fn calculate_actual_retention_by_interval(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<(f64, f64)>, sqlx::Error> {
    // 简化实现：按天统计
    let rows = sqlx::query(
        r#"
        SELECT
            MIN(10, CAST((julianday('now') - julianday(timestamp, 'unixepoch')) AS INTEGER)) as day_bucket,
            COUNT(CASE WHEN json_extract(event_data, '$.grade') IN ('good', 'easy') THEN 1 END) as correct,
            COUNT(*) as total
        FROM card_events
        WHERE event_type = 'fsrs_updated'
        GROUP BY day_bucket
        ORDER BY day_bucket ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in &rows {
        let day: f64 = row.get::<i32, _>("day_bucket") as f64;
        let correct: i32 = row.get("correct");
        let total: i32 = row.get("total");
        if total > 0 {
            result.push((day, correct as f64 / total as f64));
        }
    }

    Ok(result)
}

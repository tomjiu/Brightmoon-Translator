// Statistics Commands - 学习统计 API

use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;

/// 学习统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningStatistics {
    pub total_cards: i32,
    pub due_cards: i32,
    pub learned_today: i32,
    pub reviewed_today: i32,
    pub streak_days: i32,
    pub total_reviews: i32,
    pub retention_rate: f64,
    pub avg_daily_new: f64,
    pub avg_daily_review: f64,
}

/// 每日学习数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivity {
    pub date: String, // YYYY-MM-DD
    pub new_cards: i32,
    pub reviewed_cards: i32,
    pub time_spent: i32, // 秒
    pub correct_rate: f64,
}

/// 学习热力图数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatmapData {
    pub date: String,
    pub count: i32,
}

/// 薄弱词条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeakWord {
    pub word: String,
    pub again_count: i32,
    pub total_reviews: i32,
    pub last_review: i64,
    pub difficulty: f64,
    pub stability: f64,
}

/// 获取学习统计概览
#[tauri::command]
pub async fn get_learning_statistics(
    state: State<'_, crate::AppState>,
) -> Result<LearningStatistics, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let now = chrono::Utc::now().timestamp();
    let today_start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    // 总卡牌数
    let total_cards: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM cards")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    // 待复习卡牌数
    let due_cards: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cards WHERE json_extract(fsrs_state, '$.next_review') <= ?",
    )
    .bind(now)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    // 今日新学卡牌数（通过 card_events 统计）
    let learned_today: i32 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT card_id) FROM card_events
         WHERE event_type = 'word_imported' AND timestamp >= ?",
    )
    .bind(today_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    // 今日复习卡牌数（通过 card_events 统计 fsrs_updated）
    let reviewed_today: i32 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT card_id) FROM card_events
         WHERE event_type = 'fsrs_updated' AND timestamp >= ?",
    )
    .bind(today_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    // 连续学习天数（简化版：统计最近N天有学习记录的连续天数）
    let streak_days = calculate_streak_days(pool).await.unwrap_or(0);

    // 总复习次数
    let total_reviews: i32 =
        sqlx::query_scalar("SELECT COUNT(*) FROM card_events WHERE event_type = 'fsrs_updated'")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    // 记忆保持率（Good/Easy 占比）
    let retention_rate = calculate_retention_rate(pool).await.unwrap_or(0.0);

    // 平均每日新学
    let avg_daily_new = calculate_avg_daily_new(pool).await.unwrap_or(0.0);

    // 平均每日复习
    let avg_daily_review = calculate_avg_daily_review(pool).await.unwrap_or(0.0);

    Ok(LearningStatistics {
        total_cards,
        due_cards,
        learned_today,
        reviewed_today,
        streak_days,
        total_reviews,
        retention_rate,
        avg_daily_new,
        avg_daily_review,
    })
}

/// 获取每日学习活动（最近N天）
#[tauri::command]
pub async fn get_daily_activity(
    state: State<'_, crate::AppState>,
    days: i32,
) -> Result<Vec<DailyActivity>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let start_date = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let start_timestamp = start_date.timestamp();

    let rows = sqlx::query(
        r#"
        SELECT
            date(timestamp, 'unixepoch') as date,
            COUNT(CASE WHEN event_type = 'word_imported' THEN 1 END) as new_cards,
            COUNT(CASE WHEN event_type = 'fsrs_updated' THEN 1 END) as reviewed_cards,
            CASE WHEN COUNT(CASE WHEN event_type = 'fsrs_updated' THEN 1 END) > 0
                 THEN CAST(COUNT(CASE WHEN event_type = 'fsrs_updated' AND json_extract(event_data, '$.grade') IN ('good', 'easy') THEN 1 END) AS FLOAT)
                      / COUNT(CASE WHEN event_type = 'fsrs_updated' THEN 1 END) * 100.0
                 ELSE 0.0
            END as correct_rate
        FROM card_events
        WHERE timestamp >= ?
        GROUP BY date(timestamp, 'unixepoch')
        ORDER BY date ASC
        "#,
    )
    .bind(start_timestamp)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let activities: Vec<DailyActivity> = rows
        .into_iter()
        .map(|row| DailyActivity {
            date: row.get("date"),
            new_cards: row.get("new_cards"),
            reviewed_cards: row.get("reviewed_cards"),
            time_spent: 0,
            correct_rate: row.get("correct_rate"),
        })
        .collect();

    Ok(activities)
}

/// 获取学习热力图数据（最近365天）
#[tauri::command]
pub async fn get_heatmap_data(
    state: State<'_, crate::AppState>,
    year: i32,
) -> Result<Vec<HeatmapData>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let start_date = format!("{}-01-01", year);
    let end_date = format!("{}-12-31", year);

    let rows = sqlx::query(
        r#"
        SELECT
            date(timestamp, 'unixepoch') as date,
            COUNT(DISTINCT card_id) as count
        FROM card_events
        WHERE event_type IN ('word_imported', 'fsrs_updated')
          AND date(timestamp, 'unixepoch') BETWEEN ? AND ?
        GROUP BY date(timestamp, 'unixepoch')
        ORDER BY date ASC
        "#,
    )
    .bind(&start_date)
    .bind(&end_date)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let heatmap: Vec<HeatmapData> = rows
        .into_iter()
        .map(|row| HeatmapData {
            date: row.get("date"),
            count: row.get("count"),
        })
        .collect();

    Ok(heatmap)
}

/// 获取薄弱词列表（按错误率排序）
#[tauri::command]
pub async fn get_weak_words(
    state: State<'_, crate::AppState>,
    limit: i32,
) -> Result<Vec<WeakWord>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let rows = sqlx::query(
        r#"
        SELECT
            c.word,
            c.fsrs_state,
            COUNT(CASE WHEN json_extract(e.event_data, '$.grade') = 'again' THEN 1 END) as again_count,
            COUNT(*) as total_reviews,
            MAX(e.timestamp) as last_review
        FROM cards c
        JOIN card_events e ON c.id = e.card_id
        WHERE e.event_type = 'fsrs_updated'
        GROUP BY c.id
        HAVING total_reviews >= 3 AND again_count > 0
        ORDER BY CAST(again_count AS FLOAT) / total_reviews DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let weak_words: Vec<WeakWord> = rows
        .into_iter()
        .map(|row| {
            let fsrs_state: String = row.get("fsrs_state");
            let state: serde_json::Value = serde_json::from_str(&fsrs_state).unwrap_or_default();

            WeakWord {
                word: row.get("word"),
                again_count: row.get("again_count"),
                total_reviews: row.get("total_reviews"),
                last_review: row.get("last_review"),
                difficulty: state["difficulty"].as_f64().unwrap_or(0.0),
                stability: state["stability"].as_f64().unwrap_or(0.0),
            }
        })
        .collect();

    Ok(weak_words)
}

/// 记忆保留率曲线数据点（按首次学习后的间隔天数分桶）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPoint {
    /// 间隔天数（从首次学习起）
    pub interval_days: i32,
    /// 该桶内正确率（Good/Easy 占比 %）
    pub retention: f64,
    /// 该桶内复习次数
    pub review_count: i32,
}

/// 获取记忆保留率曲线（最近若干天内发生的复习，按学习间隔分桶）
#[tauri::command]
pub async fn get_retention_curve(
    state: State<'_, crate::AppState>,
    days: i32,
) -> Result<Vec<RetentionPoint>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let start_timestamp =
        (chrono::Utc::now() - chrono::Duration::days(days as i64)).timestamp();

    // 关联每张卡的首次学习时间与历次复习，计算间隔天数
    let rows = sqlx::query(
        r#"
        SELECT
            CAST((e.timestamp - first_seen.t) / 86400.0 AS INTEGER) as interval_days,
            json_extract(e.event_data, '$.grade') as grade
        FROM card_events e
        JOIN (
            SELECT card_id, MIN(timestamp) as t
            FROM card_events
            WHERE event_type = 'word_imported'
            GROUP BY card_id
        ) first_seen ON e.card_id = first_seen.card_id
        WHERE e.event_type = 'fsrs_updated'
          AND e.timestamp >= ?
          AND e.timestamp > first_seen.t
        "#,
    )
    .bind(start_timestamp)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 按间隔分桶：0, 1, 2-3, 4-6, 7-13, 14-20, 21-27, 28+
    let mut buckets: std::collections::HashMap<i32, (i32, i32)> = std::collections::HashMap::new(); // interval_bucket -> (correct, total)
    let bucket_of = |days: i32| -> i32 {
        match days {
            d if d <= 0 => 0,
            d if d <= 1 => 1,
            d if d <= 3 => 2,
            d if d <= 6 => 3,
            d if d <= 13 => 4,
            d if d <= 20 => 5,
            d if d <= 27 => 6,
            _ => 7,
        }
    };

    for row in rows {
        use sqlx::Row;
        let interval_days: i64 = row.get("interval_days");
        let grade: Option<String> = row.get("grade");
        let bucket = bucket_of(interval_days as i32);
        let entry = buckets.entry(bucket).or_insert((0, 0));
        entry.1 += 1;
        if matches!(grade.as_deref(), Some("good") | Some("easy")) {
            entry.0 += 1;
        }
    }

    let bucket_labels: [i32; 8] = [0, 1, 2, 4, 7, 14, 21, 28];
    let mut points: Vec<RetentionPoint> = (0..8)
        .filter_map(|b| {
            let (correct, total) = buckets.get(&b).copied().unwrap_or((0, 0));
            if total == 0 {
                return None;
            }
            Some(RetentionPoint {
                interval_days: bucket_labels[b as usize],
                retention: (correct as f64 / total as f64) * 100.0,
                review_count: total,
            })
        })
        .collect();

    points.sort_by(|a, b| a.interval_days.cmp(&b.interval_days));
    Ok(points)
}

/// 未来复习量预测点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastPoint {
    pub date: String, // YYYY-MM-DD
    pub due_count: i32,
}

/// 获取未来 N 天的到期复习量预测（基于 FSRS next_review）
#[tauri::command]
pub async fn get_review_forecast_stats(
    state: State<'_, crate::AppState>,
    days: i32,
) -> Result<Vec<ForecastPoint>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let today = chrono::Utc::now().date_naive();
    let today_start = today.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    let window_end = (today + chrono::Duration::days(days as i64))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    // T10 修复:单次聚合查询按天分桶，消除 N+1（原先逐日发 days 次 COUNT）
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT CAST(
                    (json_extract(fsrs_state, '$.next_review') - ?) / 86400
                 AS INTEGER) AS day_offset,
                COUNT(*)
         FROM cards
         WHERE json_extract(fsrs_state, '$.next_review') >= ?
           AND json_extract(fsrs_state, '$.next_review') < ?
         GROUP BY day_offset",
    )
    .bind(today_start)
    .bind(today_start)
    .bind(window_end)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("统计到期卡片失败: {e}"))?;

    let mut points = Vec::with_capacity(days as usize);
    let mut counts: std::collections::HashMap<i64, i64> =
        rows.into_iter().collect();
    for offset in 0..days {
        let day = today + chrono::Duration::days(offset as i64);
        let due = counts.remove(&(offset as i64)).unwrap_or(0);
        points.push(ForecastPoint {
            date: day.format("%Y-%m-%d").to_string(),
            due_count: due as i32,
        });
    }

    Ok(points)
}

// ============================================
// 辅助函数
// ============================================

/// 计算连续学习天数（单次聚合查询优化）
async fn calculate_streak_days(pool: &sqlx::SqlitePool) -> Result<i32, sqlx::Error> {
    let today = chrono::Utc::now().date_naive();

    // 获取最近365天有学习记录的所有日期
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT date(timestamp, 'unixepoch') as date_str
         FROM card_events
         WHERE timestamp >= ?
         ORDER BY date_str DESC",
    )
    .bind(
        (today - chrono::Duration::days(365))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp(),
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(0);
    }

    let mut streak = 0;
    let mut check_date = today;

    for date_str in &rows {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            if date == check_date {
                streak += 1;
                check_date -= chrono::Duration::days(1);
            } else if date < check_date {
                // 有缺失日期，中断连续
                break;
            }
        }
    }

    Ok(streak)
}

/// 计算记忆保持率
async fn calculate_retention_rate(pool: &sqlx::SqlitePool) -> Result<f64, sqlx::Error> {
    let row: (i32, i32) = sqlx::query_as(
        r#"
        SELECT
            COUNT(CASE WHEN json_extract(event_data, '$.grade') IN ('good', 'easy') THEN 1 END) as correct,
            COUNT(*) as total
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

/// 计算平均每日新学
async fn calculate_avg_daily_new(pool: &sqlx::SqlitePool) -> Result<f64, sqlx::Error> {
    let days = 30; // 最近30天
    let start_date = chrono::Utc::now() - chrono::Duration::days(days);
    let start_timestamp = start_date.timestamp();

    let total: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM card_events
         WHERE event_type = 'word_imported' AND timestamp >= ?",
    )
    .bind(start_timestamp)
    .fetch_one(pool)
    .await?;

    Ok(total as f64 / days as f64)
}

/// 计算平均每日复习
async fn calculate_avg_daily_review(pool: &sqlx::SqlitePool) -> Result<f64, sqlx::Error> {
    let days = 30; // 最近30天
    let start_date = chrono::Utc::now() - chrono::Duration::days(days);
    let start_timestamp = start_date.timestamp();

    let total: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM card_events
         WHERE event_type = 'fsrs_updated' AND timestamp >= ?",
    )
    .bind(start_timestamp)
    .fetch_one(pool)
    .await?;

    Ok(total as f64 / days as f64)
}

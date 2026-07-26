// Notification Commands - 学习提醒通知

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    pub enabled: bool,
    pub daily_reminder_time: String, // HH:MM format
    pub due_cards_threshold: i32,
    pub milestone_enabled: bool,
}

/// 发送桌面通知
#[tauri::command]
pub async fn send_desktop_notification(
    _app_handle: AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    // 使用系统命令发送通知（跨平台兼容）
    #[cfg(target_os = "windows")]
    {
        // Windows 使用 PowerShell
        let script = format!(
            r#"
            [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
            [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
            $template = @"
            <toast>
                <visual>
                    <binding template='ToastText02'>
                        <text id='1'>{}</text>
                        <text id='2'>{}</text>
                    </binding>
                </visual>
            </toast>
"@
            $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
            $xml.LoadXml($template)
            $toast = New-Object Windows.UI.Notifications.ToastNotification $xml
            [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("MoonTranslator").Show($toast)
            "#,
            title, body
        );

        let _ = std::process::Command::new("powershell")
            .args(["-Command", &script])
            .output();
    }

    #[cfg(target_os = "macos")]
    {
        // macOS 使用 osascript
        let script = format!(
            r#"display notification "{}" with title "{}""#,
            body.replace('"', r#"\""#),
            title.replace('"', r#"\""#)
        );
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output();
    }

    #[cfg(target_os = "linux")]
    {
        // Linux 使用 notify-send
        let _ = std::process::Command::new("notify-send")
            .arg(&title)
            .arg(&body)
            .output();
    }

    tracing::info!("📢 发送通知: {} - {}", title, body);
    Ok(())
}

/// 检查并发送每日学习提醒
#[tauri::command]
pub async fn check_daily_reminder(
    state: tauri::State<'_, crate::AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    // 获取今日学习统计
    let today_start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    let learned_today: i32 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT card_id) FROM card_events
         WHERE event_type = 'word_imported' AND timestamp >= ?",
    )
    .bind(today_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let reviewed_today: i32 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT card_id) FROM card_events
         WHERE event_type = 'fsrs_updated' AND timestamp >= ?",
    )
    .bind(today_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    // 如果今天还没学习，发送提醒
    if learned_today == 0 && reviewed_today == 0 {
        send_desktop_notification(
            app_handle,
            "📚 学习提醒".to_string(),
            "今天还没开始学习哦！坚持每天学习，养成好习惯 💪".to_string(),
        )
        .await?;
    }

    Ok(())
}

/// 检查待复习卡牌并提醒
#[tauri::command]
pub async fn check_due_cards_reminder(
    state: tauri::State<'_, crate::AppState>,
    app_handle: AppHandle,
    threshold: i32,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let now = chrono::Utc::now().timestamp();

    let due_count: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cards WHERE json_extract(fsrs_state, '$.next_review') <= ?",
    )
    .bind(now)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if due_count >= threshold {
        send_desktop_notification(
            app_handle,
            "⏰ 复习提醒".to_string(),
            format!(
                "有 {} 个单词等待复习！趁记忆还清晰，赶紧巩固一下吧 📖",
                due_count
            ),
        )
        .await?;
    }

    Ok(())
}

/// 检查学习里程碑并庆祝
#[tauri::command]
pub async fn check_milestone_celebration(
    state: tauri::State<'_, crate::AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    // 计算连续学习天数
    let streak_days = calculate_streak_days(pool).await.unwrap_or(0);

    // 里程碑天数
    let milestones = vec![3, 7, 14, 30, 60, 100, 365];

    if milestones.contains(&streak_days) {
        let emoji = match streak_days {
            3 => "🎉",
            7 => "🔥",
            14 => "⭐",
            30 => "🏆",
            60 => "💎",
            100 => "👑",
            365 => "🎊",
            _ => "✨",
        };

        send_desktop_notification(
            app_handle,
            format!("{} 学习里程碑！", emoji),
            format!(
                "恭喜你！已连续学习 {} 天！坚持就是胜利，继续加油！{}",
                streak_days, emoji
            ),
        )
        .await?;
    }

    Ok(())
}

/// 检查学习计划完成度提醒
#[tauri::command]
pub async fn check_plan_progress_reminder(
    state: tauri::State<'_, crate::AppState>,
    app_handle: AppHandle,
    plan_id: String,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    // 获取计划信息
    let plan: Option<(String, i32, i32)> =
        sqlx::query_as("SELECT name, total_words, daily_target FROM learning_plans WHERE id = ?")
            .bind(&plan_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    if let Some((name, total, daily_target)) = plan {
        // 统计已学习单词数
        let learned: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM plan_words WHERE plan_id = ? AND learned = 1")
                .bind(&plan_id)
                .fetch_one(pool)
                .await
                .unwrap_or(0);

        let completion_rate = (learned as f64 / total as f64) * 100.0;

        // 每完成 25% 提醒一次
        let milestones = vec![25.0, 50.0, 75.0, 100.0];
        for milestone in milestones {
            if (completion_rate - milestone).abs() < 1.0 {
                let emoji = match milestone as i32 {
                    25 => "🌱",
                    50 => "🌿",
                    75 => "🌳",
                    100 => "🎉",
                    _ => "✨",
                };

                send_desktop_notification(
                    app_handle.clone(),
                    format!("{} 计划进度更新", emoji),
                    format!(
                        "「{}」已完成 {}%！已学习 {}/{} 个单词 {}",
                        name, milestone as i32, learned, total, emoji
                    ),
                )
                .await?;

                break;
            }
        }

        // 今日目标提醒
        let today_start = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();

        let today_learned: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT card_id) FROM card_events ce
             JOIN plan_words pw ON ce.card_id = (SELECT id FROM cards WHERE word = pw.word)
             WHERE pw.plan_id = ? AND ce.event_type = 'word_imported' AND ce.timestamp >= ?",
        )
        .bind(&plan_id)
        .bind(today_start)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        if today_learned < daily_target as i64 {
            let remaining = daily_target as i64 - today_learned;
            send_desktop_notification(
                app_handle,
                "🎯 今日学习目标".to_string(),
                format!("「{}」今日还需学习 {} 个单词才能达标！", name, remaining),
            )
            .await?;
        }
    }

    Ok(())
}

// ============================================
// 辅助函数
// ============================================

/// 计算连续学习天数（单次聚合查询优化）
async fn calculate_streak_days(pool: &sqlx::SqlitePool) -> Result<i32, sqlx::Error> {
    let today = chrono::Utc::now().date_naive();

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
                break;
            }
        }
    }

    Ok(streak)
}

// Data Export/Import Commands - 学习数据导入导出

use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;

/// 导出卡牌记录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCard {
    pub word: String,
    pub fsrs_state: serde_json::Value,
    pub ai_content: Option<String>,
    pub created_at: i64,
    pub event_count: i32,
    pub last_review: Option<i64>,
}

/// 导出格式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportData {
    pub version: String,
    pub exported_at: i64,
    pub total_cards: i32,
    pub cards: Vec<ExportCard>,
    pub daily_activity: Vec<DailyActivityRow>,
}

/// Anki 导出格式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnkiNote {
    pub front: String, // 单词
    pub back: String,  // 释义/AI内容
    pub tags: Vec<String>,
    pub interval: i64,    // 间隔天数
    pub ease_factor: f64, // 难度系数
    pub reps: i32,
    pub lapses: i32,
}

/// Quizlet/扇贝导入格式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWord {
    pub word: String,
    pub definition: String,
    pub example: Option<String>,
}

/// 统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivityRow {
    pub date: String,
    pub new_cards: i32,
    pub reviewed_cards: i32,
}

/// 导出全部学习数据为 JSON
#[tauri::command]
pub async fn export_learning_data_json(
    state: State<'_, crate::AppState>,
) -> Result<ExportData, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let now = chrono::Utc::now().timestamp();

    // 使用聚合查询避免 N+1 问题
    let rows = sqlx::query(
        r#"
        SELECT
            c.id, c.word, c.fsrs_state, c.ai_content, c.created_at,
            COUNT(e.id) as event_count,
            MAX(CASE WHEN e.event_type = 'fsrs_updated' THEN e.timestamp END) as last_review
        FROM cards c
        LEFT JOIN card_events e ON c.id = e.card_id
        GROUP BY c.id
        ORDER BY c.created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut cards = Vec::new();
    for row in &rows {
        let fsrs_str: String = row.get("fsrs_state");
        let fsrs_state: serde_json::Value = serde_json::from_str(&fsrs_str).unwrap_or_default();

        cards.push(ExportCard {
            word: row.get("word"),
            fsrs_state,
            ai_content: row.get("ai_content"),
            created_at: row.get("created_at"),
            event_count: row.get("event_count"),
            last_review: row.get("last_review"),
        });
    }

    // 最近30天活动
    let start = now - 30 * 86400;
    let activity_rows = sqlx::query(
        "SELECT date(timestamp, 'unixepoch') as date,
                COUNT(CASE WHEN event_type = 'word_imported' THEN 1 END) as new_cards,
                COUNT(CASE WHEN event_type = 'fsrs_updated' THEN 1 END) as reviewed_cards
         FROM card_events WHERE timestamp >= ?
         GROUP BY date(timestamp, 'unixepoch') ORDER BY date ASC",
    )
    .bind(start)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let daily_activity: Vec<DailyActivityRow> = activity_rows
        .into_iter()
        .map(|r| DailyActivityRow {
            date: r.get("date"),
            new_cards: r.get("new_cards"),
            reviewed_cards: r.get("reviewed_cards"),
        })
        .collect();

    Ok(ExportData {
        version: "1.0".to_string(),
        exported_at: now,
        total_cards: cards.len() as i32,
        cards,
        daily_activity,
    })
}

/// 导出为 Anki TSV 格式
#[tauri::command]
pub async fn export_anki_tsv(state: State<'_, crate::AppState>) -> Result<Vec<AnkiNote>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let rows = sqlx::query("SELECT id, word, fsrs_state, ai_content FROM cards")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut notes = Vec::new();

    for row in &rows {
        let _card_id: String = row.get("id");
        let word: String = row.get("word");
        let fsrs_str: String = row.get("fsrs_state");
        let ai_content_str: Option<String> = row.get("ai_content");

        let fsrs_state: serde_json::Value = serde_json::from_str(&fsrs_str).unwrap_or_default();

        // 计算 Anki 格式的参数
        let stability = fsrs_state["stability"].as_f64().unwrap_or(1.0);
        let reps = fsrs_state["reps"].as_i64().unwrap_or(0) as i32;
        let lapses = fsrs_state["lapses"].as_i64().unwrap_or(0) as i32;
        let interval_days = (stability / 86400.0).max(1.0) as i64;

        // 构造背面内容
        let mut back_parts = Vec::new();

        if let Some(ai_str) = &ai_content_str {
            if let Ok(ai) = serde_json::from_str::<serde_json::Value>(ai_str) {
                if let Some(mnemonics) = ai["mnemonics"].as_str() {
                    if !mnemonics.is_empty() {
                        back_parts.push(format!("<b>助记：</b>{}", mnemonics));
                    }
                }
                if let Some(tips) = ai["tips"].as_str() {
                    if !tips.is_empty() {
                        back_parts.push(format!("<b>技巧：</b>{}", tips));
                    }
                }
                if let Some(examples) = ai["examples"].as_array() {
                    let ex: Vec<&str> = examples.iter().filter_map(|e| e.as_str()).collect();
                    if !ex.is_empty() {
                        back_parts.push(format!("<b>例句：</b><br>{}", ex.join("<br>")));
                    }
                }
            }
        }

        // 从 ECDICT 获取释义
        if let Some(ecdict_pool) = state.ecdict_pool.as_ref() {
            let def: Option<String> =
                sqlx::query_scalar("SELECT translation FROM stardict WHERE word = ? LIMIT 1")
                    .bind(&word)
                    .fetch_one(ecdict_pool)
                    .await
                    .unwrap_or(None);

            if let Some(d) = def {
                let short_def: String = d.lines().take(3).collect::<Vec<&str>>().join(" | ");
                back_parts.insert(0, format!("<b>释义：</b>{}", short_def));
            }
        }

        let tags = if reps > 10 {
            vec!["mature".to_string()]
        } else if reps > 0 {
            vec!["review".to_string()]
        } else {
            vec!["new".to_string()]
        };

        notes.push(AnkiNote {
            front: word,
            back: if back_parts.is_empty() {
                "（暂无内容）".to_string()
            } else {
                back_parts.join("<br><br>")
            },
            tags,
            interval: interval_days,
            ease_factor: 2.5,
            reps,
            lapses,
        });
    }

    Ok(notes)
}

/// 从 JSON 文件导入学习数据
#[tauri::command]
pub async fn import_learning_data_json(
    state: State<'_, crate::AppState>,
    file_path: String,
) -> Result<ImportResult, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let content =
        std::fs::read_to_string(&file_path).map_err(|e| format!("读取文件失败: {}", e))?;

    let data: ExportData =
        serde_json::from_str(&content).map_err(|e| format!("解析JSON失败: {}", e))?;

    let mut imported = 0;
    let mut skipped = 0;

    for card in &data.cards {
        // 检查是否已存在
        let exists: Option<String> = sqlx::query_scalar("SELECT id FROM cards WHERE word = ?")
            .bind(&card.word)
            .fetch_optional(store.pool())
            .await
            .unwrap_or(None);

        if exists.is_some() {
            skipped += 1;
            continue;
        }

        // 创建卡牌事件
        let card_id = uuid::Uuid::new_v4().to_string();
        let event = crate::domain::CardEvent::WordImported {
            word: card.word.clone(),
            source: "import".to_string(),
            timestamp: card.created_at,
        };
        store
            .append_event(&card_id, &event)
            .await
            .map_err(|e| e.to_string())?;

        // 如果有 AI 内容，添加 AI 事件
        if let Some(ai_str) = &card.ai_content {
            if let Ok(ai_content) = serde_json::from_str::<crate::domain::AiContent>(ai_str) {
                let ai_event = crate::domain::CardEvent::AiContentGenerated {
                    content: ai_content,
                    model: "import".to_string(),
                    confidence: 1.0,
                    timestamp: card.created_at,
                };
                store.append_event(&card_id, &ai_event).await.ok();
            }
        }

        imported += 1;
    }

    Ok(ImportResult {
        imported,
        skipped,
        total: data.cards.len() as i32,
    })
}

/// 从 CSV/TSV 导入单词列表（兼容 Quizlet/扇贝/Anki 导出）
#[tauri::command]
pub async fn import_wordlist_csv(
    state: State<'_, crate::AppState>,
    file_path: String,
) -> Result<ImportResult, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let content =
        std::fs::read_to_string(&file_path).map_err(|e| format!("读取文件失败: {}", e))?;

    let delimiter = if content.contains('\t') { '\t' } else { ',' };
    let mut imported = 0;
    let mut skipped = 0;
    let now = chrono::Utc::now().timestamp();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, delimiter).collect();
        if parts.is_empty() {
            continue;
        }

        let word = parts[0].trim().to_lowercase();
        let definition = parts
            .get(1)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if word.is_empty() || word.len() < 2 {
            continue;
        }

        // 检查是否已存在
        let exists: Option<String> = sqlx::query_scalar("SELECT id FROM cards WHERE word = ?")
            .bind(&word)
            .fetch_optional(store.pool())
            .await
            .unwrap_or(None);

        if exists.is_some() {
            skipped += 1;
            continue;
        }

        let card_id = uuid::Uuid::new_v4().to_string();
        let event = crate::domain::CardEvent::WordImported {
            word: word.clone(),
            source: "csv_import".to_string(),
            timestamp: now,
        };
        store
            .append_event(&card_id, &event)
            .await
            .map_err(|e| e.to_string())?;

        // 将释义作为 AI 内容的一部分存储
        if !definition.is_empty() {
            let ai_content = crate::domain::AiContent {
                etymology: None,
                mnemonics: vec![crate::domain::Mnemonic {
                    mnemonic_type: crate::domain::MnemonicType::Etymology,
                    content: definition,
                    score: Some(0.5),
                }],
                examples: vec![],
                scenes: vec![],
                collocations: vec![],
                word_family: vec![],
                usage_tips: vec![],
                common_mistakes: vec![],
                synonyms: vec![],
                antonyms: vec![],
            };
            let ai_event = crate::domain::CardEvent::AiContentGenerated {
                content: ai_content,
                model: "import".to_string(),
                confidence: 1.0,
                timestamp: now,
            };
            store.append_event(&card_id, &ai_event).await.ok();
        }

        imported += 1;
    }

    Ok(ImportResult {
        imported,
        skipped,
        total: imported + skipped,
    })
}

/// 自动备份到指定目录
#[tauri::command]
pub async fn auto_backup(
    state: State<'_, crate::AppState>,
    backup_dir: String,
) -> Result<String, String> {
    let export_data = export_learning_data_json(state).await?;

    // 创建备份目录
    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("创建备份目录失败: {}", e))?;

    let filename = format!(
        "moontranslator_backup_{}.json",
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    );
    let file_path = std::path::Path::new(&backup_dir).join(&filename);

    let json =
        serde_json::to_string_pretty(&export_data).map_err(|e| format!("序列化失败: {}", e))?;

    std::fs::write(&file_path, json).map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// 导入结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: i32,
    pub skipped: i32,
    pub total: i32,
}

/// 写入文件内容（用于导出功能）
#[tauri::command]
pub async fn write_file_content(file_path: String, content: String) -> Result<(), String> {
    std::fs::write(&file_path, content).map_err(|e| format!("写入文件失败: {}", e))
}

/// Write raw bytes from base64 (OCR region PNG export, etc.)
#[tauri::command]
pub async fn write_file_base64(file_path: String, base64_data: String) -> Result<(), String> {
    use base64::Engine;
    let raw = base64_data
        .split(',')
        .next_back()
        .unwrap_or(base64_data.as_str());
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw.trim())
        .map_err(|e| format!("base64 decode: {}", e))?;
    std::fs::write(&file_path, bytes).map_err(|e| format!("写入文件失败: {}", e))
}

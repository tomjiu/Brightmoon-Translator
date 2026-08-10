// learning_plan_cmd.rs - 学习计划管理 + 文件导入

use crate::domain::{LearningPlan, PlanProgressStats, PlanStatus, PlanSummary, TargetExam};
use crate::tasks::BatchGenerationTask;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamWordlist {
    pub exam: String,
    pub exam_zh: String,
    pub word_count: i32,
    pub icon: String,
    pub description: String,
}

const EXAM_CONFIGS: &[(&str, &str, &str, &str, i32)] = &[
    ("cet4", "四级", "📚", "大学英语四级 · 高频核心词", 4000),
    ("cet6", "六级", "📖", "大学英语六级 · 四级+进阶", 6000),
    ("ky", "考研", "🎓", "考研英语 · 学术高频词", 6500),
    ("ielts", "雅思", "🌍", "雅思考试 · 生活+学术", 7500),
    ("toefl", "托福", "✈️", "托福考试 · 学术英语", 9000),
    ("gre", "GRE", "🏛️", "GRE · 高阶学术词汇", 12000),
];

/// 获取可用的考试词表列表
#[tauri::command]
pub async fn get_exam_wordlists() -> Result<Vec<ExamWordlist>, String> {
    Ok(EXAM_CONFIGS
        .iter()
        .map(|(tag, zh, icon, desc, count)| ExamWordlist {
            exam: tag.to_string(),
            exam_zh: zh.to_string(),
            word_count: *count,
            icon: icon.to_string(),
            description: desc.to_string(),
        })
        .collect())
}

/// 创建学习计划
#[tauri::command]
pub async fn create_learning_plan(
    state: State<'_, crate::AppState>,
    app_handle: tauri::AppHandle,
    exam: String,
    daily_target: i32,
) -> Result<String, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let ecdict_pool = state.ecdict_pool.as_ref().ok_or("ECDICT 未连接")?;

    ensure_plan_tables(pool).await?;

    // 检查是否已有同类型计划
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM learning_plans WHERE target_exam = ?1 AND status = 'active'",
    )
    .bind(&exam)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    if existing.is_some() {
        return Err("已有进行中的计划，请先删除旧计划".to_string());
    }

    let (exam_zh, max_rank) = EXAM_CONFIGS
        .iter()
        .find(|(tag, ..)| *tag == exam)
        .map_or(("自定义".to_string(), 5000), |(_, zh, .., rank)| (zh.to_string(), *rank));

    // 从 ECDICT 提取单词
    let rows =
        sqlx::query("SELECT word FROM stardict WHERE frq IS NOT NULL ORDER BY frq DESC LIMIT ?1")
            .bind(max_rank)
            .fetch_all(ecdict_pool)
            .await
            .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return Err("词表为空".to_string());
    }

    let words: Vec<String> = rows.into_iter().map(|r| r.get("word")).collect();
    let now = chrono::Utc::now().timestamp();
    let plan_id = uuid::Uuid::new_v4().to_string();

    // 插入计划
    sqlx::query(
        "INSERT INTO learning_plans (id, name, description, plan_type, target_exam, total_words, daily_target, start_date, status, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?)"
    )
    .bind(&plan_id).bind(format!("{exam_zh}词汇")).bind(format!("{}词 · 每日{}词", words.len(), daily_target))
    .bind("preset").bind(&exam).bind(words.len() as i32).bind(daily_target)
    .bind(now).bind("active").bind(now).bind(now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    // 批量插入单词
    insert_plan_words(pool, &plan_id, &words, now).await?;

    // 启动AI批量预生成任务（后台，不阻塞返回）
    start_batch_generation(state, app_handle, plan_id.clone(), words).await;

    Ok(plan_id)
}

/// 从文件导入单词并创建计划
#[tauri::command]
pub async fn import_wordlist_from_file(
    state: State<'_, crate::AppState>,
    file_path: String,
    plan_name: String,
    daily_target: i32,
) -> Result<String, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    ensure_plan_tables(pool).await?;

    // 读取文件内容
    let content = read_file_content(&file_path)?;

    // 提取英文单词
    let words = extract_english_words(&content);
    if words.is_empty() {
        return Err("文件中未找到英文单词".to_string());
    }

    let now = chrono::Utc::now().timestamp();
    let plan_id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO learning_plans (id, name, description, plan_type, target_exam, total_words, daily_target, start_date, status, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?)"
    )
    .bind(&plan_id).bind(&plan_name).bind(format!("{}词 · 文件导入", words.len()))
    .bind("imported").bind("custom").bind(words.len() as i32).bind(daily_target)
    .bind(now).bind("active").bind(now).bind(now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    insert_plan_words(pool, &plan_id, &words, now).await?;

    Ok(plan_id)
}

/// 从文本导入单词（直接粘贴）
#[tauri::command]
pub async fn import_wordlist_from_text(
    state: State<'_, crate::AppState>,
    text: String,
    plan_name: String,
    daily_target: i32,
) -> Result<String, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    ensure_plan_tables(pool).await?;

    let words = extract_english_words(&text);
    if words.is_empty() {
        return Err("未找到英文单词".to_string());
    }

    let now = chrono::Utc::now().timestamp();
    let plan_id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO learning_plans (id, name, description, plan_type, target_exam, total_words, daily_target, start_date, status, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?)"
    )
    .bind(&plan_id).bind(&plan_name).bind(format!("{}词 · 文本导入", words.len()))
    .bind("imported").bind("custom").bind(words.len() as i32).bind(daily_target)
    .bind(now).bind("active").bind(now).bind(now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    insert_plan_words(pool, &plan_id, &words, now).await?;

    Ok(plan_id)
}

/// 获取所有学习计划
#[tauri::command]
pub async fn get_learning_plans(
    state: State<'_, crate::AppState>,
) -> Result<Vec<PlanSummary>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let table_exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='learning_plans'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if table_exists == 0 {
        return Ok(vec![]);
    }

    let rows = sqlx::query(
        "SELECT id, name, description, total_words, daily_target, start_date, status, created_at FROM learning_plans WHERE status = 'active' ORDER BY created_at DESC"
    ).fetch_all(pool).await.map_err(|e| e.to_string())?;

    let mut plans = Vec::new();
    for row in rows {
        let plan_id: String = row.get("id");
        let total: i32 = row.get("total_words");
        let daily_target: i32 = row.get("daily_target");

        let learned: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM plan_words WHERE plan_id = ?1 AND learned = 1",
        )
        .bind(&plan_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        plans.push(PlanSummary {
            plan: LearningPlan {
                id: plan_id,
                name: row.get("name"),
                description: row.get("description"),
                plan_type: crate::domain::PlanType::Preset,
                target_exam: TargetExam::default(),
                total_words: total,
                daily_target,
                start_date: row.get("start_date"),
                end_date: None,
                status: PlanStatus::Active,
                created_at: row.get("created_at"),
                updated_at: row.get("created_at"),
            },
            progress: PlanProgressStats {
                total_words: total,
                learned_words: learned as i32,
                mastered_words: 0,
                remaining_words: total - learned as i32,
                completion_rate: if total > 0 {
                    learned as f64 / f64::from(total) * 100.0
                } else {
                    0.0
                },
                days_elapsed: 0,
                estimated_days_remaining: 0,
            },
            today_target: daily_target,
            today_completed: 0,
        });
    }

    Ok(plans)
}

/// 获取计划今日待学单词
#[tauri::command]
pub async fn get_plan_today_words(
    state: State<'_, crate::AppState>,
    plan_id: String,
) -> Result<Vec<String>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let daily: i32 = sqlx::query_scalar("SELECT daily_target FROM learning_plans WHERE id = ?1")
        .bind(&plan_id)
        .fetch_one(pool)
        .await
        .map_err(|_| "计划不存在".to_string())?;

    let words: Vec<String> = sqlx::query_scalar(
        "SELECT word FROM plan_words WHERE plan_id = ?1 AND learned = 0 ORDER BY word_order LIMIT ?2"
    ).bind(&plan_id).bind(daily).fetch_all(pool).await.map_err(|e| e.to_string())?;

    Ok(words)
}

/// 标记单词已学
#[tauri::command]
pub async fn mark_word_learned(
    state: State<'_, crate::AppState>,
    plan_id: String,
    word: String,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    sqlx::query("UPDATE plan_words SET learned = 1 WHERE plan_id = ?1 AND word = ?2")
        .bind(&plan_id)
        .bind(&word)
        .execute(store.pool())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 删除学习计划
#[tauri::command]
pub async fn delete_learning_plan(
    state: State<'_, crate::AppState>,
    plan_id: String,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    sqlx::query("DELETE FROM plan_words WHERE plan_id = ?1")
        .bind(&plan_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM learning_plans WHERE id = ?1")
        .bind(&plan_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ===== 辅助函数 =====

async fn ensure_plan_tables(pool: &sqlx::SqlitePool) -> Result<(), String> {
    sqlx::query("CREATE TABLE IF NOT EXISTS learning_plans (id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT, plan_type TEXT NOT NULL, target_exam TEXT NOT NULL, total_words INTEGER NOT NULL, daily_target INTEGER NOT NULL, start_date INTEGER, end_date INTEGER, status TEXT NOT NULL DEFAULT 'active', created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)")
        .execute(pool).await.map_err(|e| e.to_string())?;
    sqlx::query("CREATE TABLE IF NOT EXISTS plan_words (plan_id TEXT NOT NULL, word TEXT NOT NULL, word_order INTEGER NOT NULL, learned INTEGER NOT NULL DEFAULT 0, added_at INTEGER NOT NULL, PRIMARY KEY (plan_id, word))")
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn insert_plan_words(
    pool: &sqlx::SqlitePool,
    plan_id: &str,
    words: &[String],
    now: i64,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for (i, word) in words.iter().enumerate() {
        sqlx::query("INSERT OR IGNORE INTO plan_words (plan_id, word, word_order, learned, added_at) VALUES (?,?,?,?,?)")
            .bind(plan_id).bind(word).bind(i as i32).bind(0).bind(now)
            .execute(&mut *tx).await.ok();
        if (i + 1) % 1000 == 0 {
            tx.commit().await.map_err(|e| e.to_string())?;
            tx = pool.begin().await.map_err(|e| e.to_string())?;
        }
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// 读取文件内容（支持 txt, md, docx, pdf）
fn read_file_content(path: &str) -> Result<String, String> {
    let path_lower = path.to_lowercase();

    if path_lower.ends_with(".txt")
        || path_lower.ends_with(".md")
        || path_lower.ends_with(".csv")
        || path_lower.ends_with(".tsv")
    {
        std::fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))
    } else if path_lower.ends_with(".docx") {
        read_docx(path)
    } else if path_lower.ends_with(".pdf") {
        read_pdf(path)
    } else {
        // 尝试作为纯文本读取
        std::fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))
    }
}

fn read_docx(path: &str) -> Result<String, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut doc = archive
        .by_name("word/document.xml")
        .map_err(|e| e.to_string())?;
    let mut content = String::new();
    doc.read_to_string(&mut content)
        .map_err(|e| e.to_string())?;

    // 简单 XML 解析：提取 <w:t> 标签内容
    let mut text = String::new();
    let mut in_text = false;
    for segment in content.split('<') {
        if let Some(rest) = segment.strip_prefix("w:t>") {
            in_text = true;
            if let Some(end) = rest.find('<') {
                text.push_str(&rest[..end]);
                text.push(' ');
            }
        } else if segment.starts_with("/w:t>") {
            in_text = false;
        } else if in_text {
            if let Some(end) = segment.find('<') {
                text.push_str(&segment[..end]);
                text.push(' ');
            }
        }
    }
    Ok(text)
}

fn read_pdf(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let text = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| e.to_string())?;
    Ok(text)
}

/// 从文本中提取去重后的英文单词
fn extract_english_words(text: &str) -> Vec<String> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut words = Vec::new();

    for token in text.split(|c: char| !c.is_ascii_alphabetic() && c != '-' && c != '\'') {
        let word = token.trim().to_lowercase();
        // 只保留 2+ 字母的纯英文单词
        if word.len() >= 2
            && word.chars().all(|c| c.is_ascii_alphabetic())
            && seen.insert(word.clone())
        {
            words.push(word);
        }
    }

    words
}

/// 启动AI批量预生成任务（后台异步执行）
async fn start_batch_generation(
    state: State<'_, crate::AppState>,
    app_handle: tauri::AppHandle,
    plan_id: String,
    words: Vec<String>,
) {
    // 获取LLM配置
    let config = state.system.config.lock().await;
    let llm_config = &config.llm;

    let api_key = if !llm_config.api_key.is_empty() {
        llm_config.api_key.clone()
    } else if let Some(key) = llm_config.api_keys.first() {
        key.clone()
    } else {
        tracing::warn!("未配置 LLM API Key，跳过AI内容批量生成");
        return;
    };

    let base_url = llm_config.base_url.clone();
    let model = llm_config.model.clone();
    drop(config);

    if base_url.is_empty() {
        tracing::warn!("未配置 LLM Base URL，跳过AI内容批量生成");
        return;
    }

    let event_store = if let Some(store) = state.event_store.as_ref() { Arc::new(store.clone()) } else {
        tracing::warn!("数据库未初始化，跳过AI内容批量生成");
        return;
    };

    // 只生成前100个单词（避免API费用过高）
    let words_to_generate: Vec<String> = words.into_iter().take(100).collect();

    tracing::info!(
        "🚀 启动AI批量预生成任务: plan_id={}, 单词数={}, model={}",
        plan_id,
        words_to_generate.len(),
        model
    );

    // 生成任务ID
    let task_id = format!("batch-{}", uuid::Uuid::new_v4());

    // 在后台spawn任务（不阻塞返回）
    tokio::spawn(async move {
        let task = BatchGenerationTask::new(
            task_id,
            words_to_generate,
            api_key,
            base_url,
            model,
            event_store,
            app_handle,
        );

        if let Err(e) = task.run().await {
            tracing::error!("AI批量预生成任务失败: {}", e);
        }
    });
}

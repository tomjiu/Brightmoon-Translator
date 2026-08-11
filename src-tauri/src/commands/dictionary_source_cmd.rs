// Dictionary Source Commands - T7 词典源管理
//
// 管理可插拔词典源（ECDICT / 有道 / 在线API / AI Prompt）：
// - get_dict_sources: 列出所有源及配置
// - update_dict_source: 更新源配置（启用/优先级/自定义 prompt 模板）
// - lookup_word_all_sources: 聚合查询所有启用源

use crate::services::dictionary_source::{DictEntryResult, DictSourceConfig, SourceRegistry};
use serde::{Deserialize, Serialize};

/// 列出所有词典源配置
#[tauri::command]
pub async fn get_dict_sources(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<DictSourceConfig>, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();

    let rows = sqlx::query(
        "SELECT id, name, enabled, priority, prompt_template FROM dictionary_sources ORDER BY priority DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut sources = Vec::new();
    for row in rows {
        use sqlx::Row;
        sources.push(DictSourceConfig {
            id: row.try_get("id").unwrap_or_default(),
            name: row.try_get("name").unwrap_or_default(),
            enabled: row.try_get::<i64, _>("enabled").unwrap_or(1) == 1,
            priority: row.try_get::<i64, _>("priority").unwrap_or(0) as i32,
            prompt_template: row.try_get("prompt_template").ok(),
        });
    }

    Ok(sources)
}

/// 更新词典源配置
#[tauri::command]
pub async fn update_dict_source(
    state: tauri::State<'_, crate::AppState>,
    source_id: String,
    enabled: Option<bool>,
    priority: Option<i32>,
    prompt_template: Option<String>,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();

    // 构建动态 UPDATE
    let mut set_clauses = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(en) = enabled {
        set_clauses.push("enabled = ?".to_string());
        binds.push(if en { "1" } else { "0" }.to_string());
    }
    if let Some(p) = priority {
        set_clauses.push("priority = ?".to_string());
        binds.push(p.to_string());
    }
    if let Some(t) = prompt_template {
        set_clauses.push("prompt_template = ?".to_string());
        binds.push(t);
    }

    if set_clauses.is_empty() {
        return Ok(());
    }

    set_clauses.push("updated_at = ?".to_string());
    binds.push(chrono::Utc::now().timestamp().to_string());

    let sql = format!(
        "UPDATE dictionary_sources SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&source_id);

    q.execute(pool).await.map_err(|e| e.to_string())?;

    Ok(())
}

/// 聚合查询所有启用源（多源结果）
#[tauri::command]
pub async fn lookup_word_all_sources(
    state: tauri::State<'_, crate::AppState>,
    word: String,
) -> Result<Vec<DictEntryResult>, String> {
    let registry = build_registry(&state).await;
    Ok(registry.lookup_all(&word).await)
}

/// 构建源注册表（从配置 + `AppState` 组装）
async fn build_registry(state: &tauri::State<'_, crate::AppState>) -> SourceRegistry {
    use crate::services::dictionary_source::{
        AiPromptSource, EcdictSource, OnlineApiSource,
    };

    let mut registry = SourceRegistry::new();

    // 读取启用的源配置
    let configs = if let Some(store) = &state.event_store {
        let pool = store.pool();
        let rows = sqlx::query(
            "SELECT id, name, enabled, priority, prompt_template FROM dictionary_sources WHERE enabled = 1 ORDER BY priority DESC",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut cfgs: Vec<(String, String, i32, Option<String>)> = Vec::new();
        for row in rows {
            use sqlx::Row;
            cfgs.push((
                row.try_get::<String, _>("id").unwrap_or_default(),
                row.try_get::<String, _>("name").unwrap_or_default(),
                row.try_get::<i64, _>("priority").unwrap_or(0) as i32,
                row.try_get::<Option<String>, _>("prompt_template").ok().flatten(),
            ));
        }
        cfgs
    } else {
        Vec::new()
    };

    // 按优先级排序
    let mut ordered = configs;
    ordered.sort_by_key(|a| std::cmp::Reverse(a.2));

    for (id, _name, _priority, template) in ordered {
        match id.as_str() {
            "ecdict" => {
                if let Some(pool) = &state.ecdict_pool {
                    registry.register(Box::new(EcdictSource::new(pool.clone())));
                }
            },
            "online_api" => {
                registry.register(Box::new(OnlineApiSource::new()));
            },
            "ai_prompt" => {
                // 从 config 读取 LLM 配置
                let config = state.system.config.lock().await;
                let llm = &config.llm;
                let api_key = if llm.api_key.is_empty() {
                    llm.api_keys.first().cloned()
                } else {
                    Some(llm.api_key.clone())
                };
                let base_url = llm.base_url.clone();
                let model = llm.model.clone();
                drop(config);

                if let (Some(key), false) = (api_key, base_url.is_empty()) {
                    let mut source = AiPromptSource::new(key, base_url, model);
                    if let Some(t) = template {
                        source = source.with_template(t);
                    }
                    registry.register(Box::new(source));
                }
            },
            "youdao" => {
                // 有道在 MultiSourceDictionary 中已有封装，此处用在线 API 兜底标记
                // （有道原始 JSON 解析在 dictionary_cmd::parse_youdao）
            },
            _ => {},
        }
    }

    // 若配置表为空（首次），默认注册 ECDICT + 在线
    if registry.list().is_empty() {
        if let Some(pool) = &state.ecdict_pool {
            registry.register(Box::new(EcdictSource::new(pool.clone())));
        }
        registry.register(Box::new(OnlineApiSource::new()));
    }

    registry
}

/// 保存自定义源配置的请求（供前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSourceRequest {
    pub source_id: String,
    pub enabled: bool,
    pub priority: i32,
    pub prompt_template: Option<String>,
}

/// 批量保存源配置
#[tauri::command]
pub async fn save_dict_sources(
    state: tauri::State<'_, crate::AppState>,
    sources: Vec<SaveSourceRequest>,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();

    for s in sources {
        sqlx::query(
            r"
            UPDATE dictionary_sources
            SET enabled = ?, priority = ?, prompt_template = ?, updated_at = ?
            WHERE id = ?
            ",
        )
        .bind(i32::from(s.enabled))
        .bind(s.priority)
        .bind(&s.prompt_template)
        .bind(chrono::Utc::now().timestamp())
        .bind(&s.source_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

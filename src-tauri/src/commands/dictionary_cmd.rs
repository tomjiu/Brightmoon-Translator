// Dictionary Commands - 词典查询相关命令（多源聚合）

use crate::app_context::AppState;
use crate::models::dictionary::DictionaryResult;
use crate::services::multi_dictionary::{DictionaryEntry, MultiSourceDictionary};
use sqlx::SqlitePool;
use tauri::State;

/// 多源词典查询（优先在线 API，兜底本地）
#[tauri::command]
pub async fn lookup_word_multi_source(
    word: String,
    state: State<'_, AppState>,
) -> Result<DictionaryEntry, String> {
    let dict = MultiSourceDictionary::new();

    // 1. 先尝试在线 API（dictionaryapi.dev）
    match dict.lookup(&word).await {
        Ok(mut entries) => {
            if !entries.is_empty() {
                return Ok(entries.remove(0));
            }
        }
        Err(e) => {
            eprintln!("Online dictionary failed: {}", e);
        }
    }

    // 2. 如果在线失败，尝试本地 ECDICT
    match lookup_word_detail_local(word, state).await {
        Ok(local_result) => {
            // 转换 DictionaryResult 到 DictionaryEntry
            Ok(convert_local_to_entry(local_result))
        }
        Err(e) => Err(format!("查询失败: {}", e)),
    }
}

/// 本地 ECDICT 查询（兜底）
async fn lookup_word_detail_local(
    word: String,
    state: State<'_, AppState>,
) -> Result<DictionaryResult, String> {
    let pool = state.db_pool.lock().await;
    if pool.is_none() {
        return Err("Database not initialized".to_string());
    }
    let pool = pool.as_ref().unwrap();

    let word_lower = word.to_lowercase();

    let result = sqlx::query!(
        r#"
        SELECT
            word,
            phonetic,
            definition,
            translation,
            pos,
            collins,
            oxford,
            tag,
            bnc,
            frq,
            exchange
        FROM ecdict
        WHERE LOWER(word) = ?1
        "#,
        word_lower
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(row) = result {
        Ok(DictionaryResult {
            word: row.word,
            phonetic: row.phonetic,
            definition: row.definition,
            translation: row.translation,
            pos: row.pos,
            collins: row.collins,
            oxford: row.oxford,
            tag: row.tag,
            bnc: row.bnc,
            frq: row.frq,
            exchange: row.exchange,
        })
    } else {
        Err(format!("Word '{}' not found in local database", word))
    }
}

/// 转换本地结果到统一格式
fn convert_local_to_entry(local: DictionaryResult) -> DictionaryEntry {
    use crate::services::multi_dictionary::{Definition, Meaning, Phonetic};

    let phonetics = if let Some(phonetic) = local.phonetic {
        vec![Phonetic {
            text: Some(phonetic),
            audio: None,
        }]
    } else {
        vec![]
    };

    let mut meanings = Vec::new();

    if let Some(definition) = local.definition {
        let defs: Vec<String> = definition.split("\\n").map(|s| s.to_string()).collect();
        meanings.push(Meaning {
            part_of_speech: local.pos.unwrap_or_else(|| "n.".to_string()),
            definitions: defs
                .into_iter()
                .map(|d| Definition {
                    definition: d,
                    example: None,
                    synonyms: vec![],
                    antonyms: vec![],
                })
                .collect(),
        });
    }

    DictionaryEntry {
        word: local.word,
        phonetics,
        meanings,
        source: "ECDICT (Local)".to_string(),
    }
}

/// 搜索建议（前缀匹配）
#[tauri::command]
pub async fn search_word_suggestions(
    query: String,
    limit: i32,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let pool = state.db_pool.lock().await;
    if pool.is_none() {
        return Err("Database not initialized".to_string());
    }
    let pool = pool.as_ref().unwrap();

    let query_lower = query.to_lowercase();
    let pattern = format!("{}%", query_lower);

    let results = sqlx::query!(
        r#"
        SELECT word
        FROM ecdict
        WHERE LOWER(word) LIKE ?1
        ORDER BY frq DESC, word ASC
        LIMIT ?2
        "#,
        pattern,
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(results.into_iter().map(|r| r.word).collect())
}

/// 详细查询单词（仅本地，用于兼容）
#[tauri::command]
pub async fn lookup_word_detail(
    word: String,
    state: State<'_, AppState>,
) -> Result<DictionaryResult, String> {
    lookup_word_detail_local(word, state).await
}

/// 模糊搜索（包含匹配）
#[tauri::command]
pub async fn fuzzy_search_words(
    query: String,
    limit: i32,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let pool = state.db_pool.lock().await;
    if pool.is_none() {
        return Err("Database not initialized".to_string());
    }
    let pool = pool.as_ref().unwrap();

    let query_lower = query.to_lowercase();
    let pattern = format!("%{}%", query_lower);

    let results = sqlx::query!(
        r#"
        SELECT word
        FROM ecdict
        WHERE LOWER(word) LIKE ?1
           OR LOWER(translation) LIKE ?1
        ORDER BY
            CASE
                WHEN LOWER(word) = ?2 THEN 0
                WHEN LOWER(word) LIKE ?3 THEN 1
                ELSE 2
            END,
            frq DESC,
            word ASC
        LIMIT ?4
        "#,
        pattern,
        query_lower,
        format!("{}%", query_lower),
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(results.into_iter().map(|r| r.word).collect())
}

// Dictionary Commands - 词典查询相关命令

use crate::app_context::AppState;
use crate::models::dictionary::DictionaryResult;
use sqlx::SqlitePool;
use tauri::State;

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

/// 详细查询单词
#[tauri::command]
pub async fn lookup_word_detail(
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
        Err(format!("Word '{}' not found", word))
    }
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

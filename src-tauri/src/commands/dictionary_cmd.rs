// Dictionary Commands - 词典查询相关命令（多源聚合）

use crate::models::dictionary::{Definition, DictionaryResult, Meaning};
use crate::services::multi_dictionary::{DictionaryEntry, MultiSourceDictionary};
use sqlx::{Row, SqlitePool};
use tauri::State;

/// 多源词典查询（优先在线 API，兜底本地）
#[tauri::command]
pub async fn lookup_word_multi_source(
    word: String,
    pool: State<'_, SqlitePool>,
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
    match lookup_word_detail_local(word, pool.inner()).await {
        Ok(local_result) => Ok(local_result),
        Err(e) => Err(format!("查询失败: {}", e)),
    }
}

/// 本地 ECDICT 查询（返回 DictionaryEntry 格式）
async fn lookup_word_detail_local(
    word: String,
    pool: &SqlitePool,
) -> Result<DictionaryEntry, String> {
    let word_lower = word.to_lowercase();

    let row = sqlx::query(
        "SELECT word, phonetic, definition, translation FROM ecdict WHERE LOWER(word) = ?1"
    )
    .bind(&word_lower)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(row) = row {
        use crate::services::multi_dictionary::Phonetic;

        let word: String = row.get("word");
        let phonetic: Option<String> = row.get("phonetic");
        let definition: Option<String> = row.get("definition");
        let translation: Option<String> = row.get("translation");

        let phonetics = if let Some(phonetic) = phonetic {
            vec![Phonetic {
                text: Some(phonetic),
                audio: None,
            }]
        } else {
            vec![]
        };

        let mut meanings = Vec::new();

        // 处理 definition 字段
        if let Some(definition) = definition {
            let defs: Vec<&str> = definition.split("\\n").collect();
            let definitions: Vec<crate::services::multi_dictionary::Definition> = defs
                .into_iter()
                .map(|d| crate::services::multi_dictionary::Definition {
                    definition: d.to_string(),
                    example: None,
                    synonyms: vec![],
                    antonyms: vec![],
                })
                .collect();

            meanings.push(crate::services::multi_dictionary::Meaning {
                part_of_speech: "词义".to_string(),
                definitions,
            });
        }

        // 处理 translation 字段
        if let Some(translation) = translation {
            let trans_defs = vec![crate::services::multi_dictionary::Definition {
                definition: translation,
                example: None,
                synonyms: vec![],
                antonyms: vec![],
            }];

            meanings.push(crate::services::multi_dictionary::Meaning {
                part_of_speech: "中文释义".to_string(),
                definitions: trans_defs,
            });
        }

        Ok(DictionaryEntry {
            word,
            phonetics,
            meanings,
            source: "ECDICT (本地)".to_string(),
        })
    } else {
        Err(format!("单词 '{}' 未在本地数据库找到", word))
    }
}

/// 搜索建议（前缀匹配）
#[tauri::command]
pub async fn search_word_suggestions(
    query: String,
    limit: i32,
    pool: State<'_, SqlitePool>,
) -> Result<Vec<String>, String> {
    let query_lower = query.to_lowercase();
    let pattern = format!("{}%", query_lower);

    let rows = sqlx::query(
        "SELECT word FROM ecdict WHERE LOWER(word) LIKE ?1 ORDER BY frq DESC, word ASC LIMIT ?2"
    )
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(|r| r.get("word")).collect())
}

/// 详细查询单词（兼容旧接口，返回 DictionaryResult）
#[tauri::command]
pub async fn lookup_word_detail(
    word: String,
    pool: State<'_, SqlitePool>,
) -> Result<DictionaryResult, String> {
    let word_lower = word.to_lowercase();

    let row = sqlx::query(
        "SELECT word, phonetic, definition, translation FROM ecdict WHERE LOWER(word) = ?1"
    )
    .bind(&word_lower)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    if let Some(row) = row {
        let word: String = row.get("word");
        let phonetic: Option<String> = row.get("phonetic");
        let definition: Option<String> = row.get("definition");
        let translation: Option<String> = row.get("translation");

        let mut meanings = Vec::new();

        // 处理 definition
        if let Some(definition) = definition {
            let defs: Vec<&str> = definition.split("\\n").collect();
            let definitions: Vec<Definition> = defs
                .into_iter()
                .map(|d| Definition {
                    definition: d.to_string(),
                    example: None,
                    synonyms: vec![],
                    antonyms: vec![],
                })
                .collect();

            meanings.push(Meaning {
                part_of_speech: "词义".to_string(),
                definitions,
            });
        }

        // 处理 translation
        if let Some(translation) = translation {
            meanings.push(Meaning {
                part_of_speech: "中文释义".to_string(),
                definitions: vec![Definition {
                    definition: translation,
                    example: None,
                    synonyms: vec![],
                    antonyms: vec![],
                }],
            });
        }

        Ok(DictionaryResult {
            word,
            phonetic,
            meanings,
            source_urls: vec![],
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
    pool: State<'_, SqlitePool>,
) -> Result<Vec<String>, String> {
    let query_lower = query.to_lowercase();
    let pattern = format!("%{}%", query_lower);
    let prefix_pattern = format!("{}%", query_lower);

    let rows = sqlx::query(
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
        "#
    )
    .bind(&pattern)
    .bind(&query_lower)
    .bind(&prefix_pattern)
    .bind(limit)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(|r| r.get("word")).collect())
}

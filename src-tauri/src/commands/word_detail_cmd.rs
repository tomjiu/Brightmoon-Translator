// Word Detail Commands - 单词详情增强 API

use crate::domain::CardEvent;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordHistory {
    pub event_type: String,
    pub timestamp: i64,
    pub rating: Option<String>,
    pub difficulty: Option<f64>,
    pub stability: Option<f64>,
    pub next_review: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsrsTimeline {
    pub date: String,
    pub difficulty: f64,
    pub stability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedWord {
    pub word: String,
    pub relation_type: String, // root, synonym, antonym
    pub definition: Option<String>,
}

/// 获取单词学习历史（时间线）
#[tauri::command]
pub async fn get_word_history(
    state: State<'_, crate::AppState>,
    word: String,
) -> Result<Vec<WordHistory>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    // 查找卡牌ID
    let card_id: Option<String> = sqlx::query_scalar("SELECT id FROM cards WHERE word = ?")
        .bind(&word)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    let card_id = card_id.ok_or("单词不存在")?;

    // 获取所有事件
    let rows = sqlx::query(
        "SELECT event_type, event_data, timestamp FROM card_events
         WHERE card_id = ? ORDER BY timestamp ASC",
    )
    .bind(&card_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut history = Vec::new();
    for row in rows {
        let event_type: String = row.get("event_type");
        let event_data: String = row.get("event_data");
        let timestamp: i64 = row.get("timestamp");

        let data: serde_json::Value = serde_json::from_str(&event_data).unwrap_or_default();

        let rating = match event_type.as_str() {
            "fsrs_updated" => data["grade"].as_str().map(std::string::ToString::to_string),
            _ => None,
        };

        let difficulty = data["fsrs_state"]["difficulty"].as_f64();
        let stability = data["fsrs_state"]["stability"].as_f64();
        let next_review = data["fsrs_state"]["next_review"].as_i64();

        history.push(WordHistory {
            event_type,
            timestamp,
            rating,
            difficulty,
            stability,
            next_review,
        });
    }

    Ok(history)
}

/// 获取 FSRS 参数变化曲线
#[tauri::command]
pub async fn get_fsrs_timeline(
    state: State<'_, crate::AppState>,
    word: String,
) -> Result<Vec<FsrsTimeline>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let card_id: Option<String> = sqlx::query_scalar("SELECT id FROM cards WHERE word = ?")
        .bind(&word)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    let card_id = card_id.ok_or("单词不存在")?;

    let rows = sqlx::query(
        "SELECT event_data, timestamp FROM card_events
         WHERE card_id = ? AND event_type = 'fsrs_updated' ORDER BY timestamp ASC",
    )
    .bind(&card_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut timeline = Vec::new();
    for row in rows {
        let event_data: String = row.get("event_data");
        let timestamp: i64 = row.get("timestamp");

        let data: serde_json::Value = serde_json::from_str(&event_data).unwrap_or_default();

        if let (Some(difficulty), Some(stability)) = (
            data["fsrs_state"]["difficulty"].as_f64(),
            data["fsrs_state"]["stability"].as_f64(),
        ) {
            let date = chrono::DateTime::from_timestamp(timestamp, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_default();

            timeline.push(FsrsTimeline {
                date,
                difficulty,
                stability,
            });
        }
    }

    Ok(timeline)
}

/// 更新AI内容（手动编辑）
#[tauri::command]
pub async fn update_ai_content(
    state: State<'_, crate::AppState>,
    word: String,
    ai_content: crate::domain::AiContent,
) -> Result<(), String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let card_id: Option<String> = sqlx::query_scalar("SELECT id FROM cards WHERE word = ?")
        .bind(&word)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    let card_id = card_id.ok_or("单词不存在")?;

    let now = chrono::Utc::now().timestamp();
    let event = CardEvent::AiContentGenerated {
        content: ai_content,
        model: "manual_edit".to_string(),
        confidence: 1.0,
        timestamp: now,
    };

    store
        .append_event(&card_id, &event)
        .await
        .map_err(|e| e.to_string())?;

    // 更新快照
    if let Ok(card) = store.rebuild_card(&card_id).await {
        store
            .update_snapshot(&card)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 获取相关词汇（同根词/近义词）
#[tauri::command]
pub async fn get_related_words(
    state: State<'_, crate::AppState>,
    word: String,
) -> Result<Vec<RelatedWord>, String> {
    let ecdict_pool = state.ecdict_pool.as_ref().ok_or("ECDICT 未连接")?;

    let mut related = Vec::new();

    // 查找同根词（简单实现：前缀匹配）
    if word.len() >= 4 {
        let prefix = &word[..word.len().min(5)];
        let rows = sqlx::query(
            "SELECT word, definition FROM stardict
             WHERE word LIKE ? AND word != ? AND frq IS NOT NULL
             ORDER BY frq ASC LIMIT 10",
        )
        .bind(format!("{prefix}%"))
        .bind(&word)
        .fetch_all(ecdict_pool)
        .await
        .unwrap_or_default();

        for row in rows {
            let related_word: String = row.get("word");
            let definition: Option<String> = row.get("definition");

            related.push(RelatedWord {
                word: related_word,
                relation_type: "root".to_string(),
                definition: definition.map(|d| d.chars().take(50).collect()),
            });
        }
    }

    Ok(related)
}

/// 获取单词在语料库中的例句
#[tauri::command]
pub async fn get_corpus_examples(
    state: State<'_, crate::AppState>,
    word: String,
    limit: i32,
) -> Result<Vec<String>, String> {
    let ecdict_pool = state.ecdict_pool.as_ref().ok_or("ECDICT 未连接")?;

    // 从 ECDICT 获取例句（tag 字段通常包含例句）
    let row: Option<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT definition, translation, tag FROM stardict
         WHERE word = ? LIMIT 1",
    )
    .bind(&word)
    .fetch_optional(ecdict_pool)
    .await
    .unwrap_or(None);

    let mut examples = Vec::new();

    if let Some((definition, translation, tag)) = row {
        // 尝试从 tag 字段提取英文例句
        if let Some(tag_str) = tag {
            for line in tag_str.lines() {
                let line = line.trim();
                if line.contains(&word)
                    && line.len() > 10
                    && line.len() < 200
                    && line.chars().any(|c| c.is_ascii_alphabetic())
                {
                    examples.push(line.to_string());
                    if examples.len() >= limit as usize {
                        return Ok(examples);
                    }
                }
            }
        }

        // 从 translation 字段提取包含该词的英文例句
        if let Some(trans) = translation {
            for line in trans.lines() {
                let line = line.trim();
                // 只选择看起来像英文例句的行（包含空格、标点，且以字母开头）
                if line.len() > 15
                    && line.len() < 200
                    && line.starts_with(|c: char| c.is_ascii_alphabetic())
                    && line.contains(' ')
                {
                    examples.push(line.to_string());
                    if examples.len() >= limit as usize {
                        return Ok(examples);
                    }
                }
            }
        }

        // 从 definition 字段提取英文例句
        if let Some(def) = definition {
            for line in def.lines() {
                let line = line.trim();
                if line.len() > 15
                    && line.len() < 200
                    && line.starts_with(|c: char| c.is_ascii_alphabetic())
                    && line.contains(' ')
                {
                    examples.push(line.to_string());
                    if examples.len() >= limit as usize {
                        return Ok(examples);
                    }
                }
            }
        }
    }

    Ok(examples)
}

/// 获取单词词根词缀分析
///
/// S5-2: 先查 morphology 表（MorphoLex 导入的词根拆解数据），
/// 查不到时 fallback 到内置的前缀/后缀启发式。morphology 表可能
/// 不存在（用户未运行 DataInitializer），所以查询失败时静默降级。
#[tauri::command]
pub async fn get_word_etymology(
    state: State<'_, crate::AppState>,
    word: String,
) -> Result<String, String> {
    // 1. 尝试从 morphology 表查询（如果数据库可用）
    if let Some(store) = state.event_store.as_ref() {
        let pool = store.pool();
        let result = sqlx::query("SELECT segmentation, pos, parts FROM morphology WHERE word = ?")
            .bind(word.to_lowercase())
            .fetch_optional(pool)
            .await;

        if let Ok(Some(row)) = result {
            let segmentation: String = row.try_get("segmentation").unwrap_or_default();
            let pos: Option<String> = row.try_get("pos").ok().flatten();
            let parts_json: String = row.try_get("parts").unwrap_or_default();

            if !segmentation.is_empty() {
                return Ok(format_morphology_result(&word, &segmentation, pos.as_deref(), &parts_json));
            }
        }
        // 查询失败（表不存在）或无结果 — 静默降级到启发式
    }

    // 2. Fallback: 内置前缀/后缀启发式
    Ok(etymology_heuristic(&word))
}

/// 格式化 morphology 表的查询结果为用户可读的词根分析。
fn format_morphology_result(
    word: &str,
    segmentation: &str,
    pos: Option<&str>,
    parts_json: &str,
) -> String {
    let mut lines = Vec::new();

    if let Some(p) = pos {
        if !p.is_empty() {
            lines.push(format!("词性: {p}"));
        }
    }

    // 尝试解析 parts JSON 获取更详细的拆解
    let parts_parsed: Vec<serde_json::Value> = serde_json::from_str(parts_json).unwrap_or_default();
    if !parts_parsed.is_empty() {
        for part in &parts_parsed {
            let part_text = part.get("part").and_then(|v| v.as_str()).unwrap_or("");
            let part_type = part.get("part_type").and_then(|v| v.as_str()).unwrap_or("root");
            let meaning = part.get("meaning").and_then(|v| v.as_str());

            let type_label = match part_type {
                "prefix" => "前缀",
                "suffix" => "后缀",
                _ => "词根",
            };

            if let Some(m) = meaning {
                if m.is_empty() {
                    lines.push(format!("{type_label}: {part_text}"));
                } else {
                    lines.push(format!("{type_label}: {part_text} ({m})"));
                }
            } else {
                lines.push(format!("{type_label}: {part_text}"));
            }
        }
    } else if !segmentation.is_empty() {
        // parts JSON 解析失败时，直接用 segmentation
        lines.push(format!("拆解: {segmentation}"));
    }

    if lines.is_empty() {
        format!("「{word}」是一个单一词根")
    } else {
        lines.join("\n")
    }
}

/// 启发式词根分析（morphology 表不可用时的 fallback）。
fn etymology_heuristic(word: &str) -> String {
    let common_prefixes = vec![
        ("un", "不，非"),
        ("re", "再，重新"),
        ("dis", "不，相反"),
        ("pre", "预先，在前"),
        ("post", "后"),
        ("anti", "反对，抗"),
        ("de", "去除，向下"),
        ("en", "使，进入"),
        ("ex", "出，外"),
        ("in", "在内，不"),
        ("inter", "在...之间"),
        ("mis", "错误，坏"),
        ("over", "过度，超过"),
        ("sub", "在下，次"),
        ("trans", "跨越，转换"),
    ];

    let common_suffixes = vec![
        ("able", "能够的，可...的"),
        ("tion", "行为，状态"),
        ("ness", "状态，性质"),
        ("ment", "行为，结果"),
        ("ly", "...地（副词）"),
        ("ful", "充满...的"),
        ("less", "没有...的"),
        ("er", "做...的人/物"),
        ("ist", "...主义者"),
        ("ize", "使...化（动词）"),
    ];

    let mut analysis = Vec::new();

    // 检查前缀
    for (prefix, meaning) in common_prefixes {
        if word.starts_with(prefix) && word.len() > prefix.len() + 2 {
            let root = &word[prefix.len()..];
            analysis.push(format!("前缀: {prefix} ({meaning})"));
            analysis.push(format!("词根: {root}"));
            break;
        }
    }

    // 检查后缀
    for (suffix, meaning) in common_suffixes {
        if word.ends_with(suffix) && word.len() > suffix.len() + 2 {
            let root = &word[..word.len() - suffix.len()];
            if analysis.is_empty() {
                analysis.push(format!("词根: {root}"));
            }
            analysis.push(format!("后缀: {suffix} ({meaning})"));
            break;
        }
    }

    if analysis.is_empty() {
        format!("「{word}」是一个单一词根")
    } else {
        analysis.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── S5-2: etymology heuristic fallback ────────────────────────────────

    #[test]
    fn heuristic_detects_prefix() {
        let result = etymology_heuristic("unhappy");
        assert!(result.contains("前缀: un"), "got: {result}");
        assert!(result.contains("词根: happy"), "got: {result}");
    }

    #[test]
    fn heuristic_detects_suffix() {
        let result = etymology_heuristic("quickly");
        assert!(result.contains("后缀: ly"), "got: {result}");
    }

    #[test]
    fn heuristic_single_root() {
        let result = etymology_heuristic("cat");
        assert!(result.contains("单一词根"), "got: {result}");
    }

    // ── S5-2: morphology result formatting ───────────────────────────────

    #[test]
    fn format_morphology_with_parts_json() {
        // Simulate the parts JSON shape produced by data_init::parse_segmentation
        let parts = r#"[{"part":"brill","part_type":"prefix","meaning":null},{"part":"i","part_type":"root","meaning":null},{"part":"ant","part_type":"suffix","meaning":null}]"#;
        let result = format_morphology_result("brilliant", "brill.i.ant", Some("adj."), parts);
        assert!(result.contains("词性: adj."), "got: {result}");
        assert!(result.contains("前缀: brill"), "got: {result}");
        assert!(result.contains("词根: i"), "got: {result}");
        assert!(result.contains("后缀: ant"), "got: {result}");
    }

    #[test]
    fn format_morphology_with_meaning() {
        let parts = r#"[{"part":"pre","part_type":"prefix","meaning":"预先"},{"part":"view","part_type":"root","meaning":"看"}]"#;
        let result = format_morphology_result("preview", "pre.view", None, parts);
        assert!(result.contains("前缀: pre (预先)"), "got: {result}");
        assert!(result.contains("词根: view (看)"), "got: {result}");
    }

    #[test]
    fn format_morphology_empty_parts_falls_back_to_segmentation() {
        // parts JSON is invalid/empty — should still show segmentation
        let result = format_morphology_result("test", "te.st", None, "[]");
        assert!(result.contains("拆解: te.st"), "got: {result}");
    }

    #[test]
    fn format_morphology_empty_everything() {
        let result = format_morphology_result("word", "", None, "[]");
        assert!(result.contains("单一词根"), "got: {result}");
    }
}

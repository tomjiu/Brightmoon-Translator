// Learning Mode Commands - 多样化学习模式 API

use crate::skills::llm_provider::extract_json;
use crate::skills::{LlmMessage, LlmProvider, LlmRequest};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;

/// 选择题
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceQuestion {
    pub word: String,
    pub question: String,
    pub options: Vec<String>,
    pub correct_index: usize,
    pub explanation: Option<String>,
}
/// 拼写题
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellingQuestion {
    pub definition: String,
    pub hint: String, // 首字母或字母数提示
    pub answer: String,
    pub example: Option<String>,
}

/// 填空题
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClozeQuestion {
    pub sentence: String, // 带 ___ 的句子
    pub answer: String,
    pub options: Vec<String>,
    pub context: Option<String>,
}

/// 生成选择题（4选1）
#[tauri::command]
pub async fn generate_choice_questions(
    state: State<'_, crate::AppState>,
    plan_id: Option<String>,
    count: i32,
) -> Result<Vec<ChoiceQuestion>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let ecdict_pool = state.ecdict_pool.as_ref();

    // 获取要测试的单词
    let words = get_quiz_words(pool, plan_id, count).await?;

    // 并发生成每道题（LLM 验证是网络请求，串行会阻塞 10 题 = 20-30s）
    let pool_clone = pool.clone();
    let state_ref = &state;
    let futures = words
        .iter()
        .map(|(word, definition)| {
            let pool = pool_clone.clone();
            let ecdict_pool = ecdict_pool.map(|p| p.clone());
            let words_clone: Vec<(String, String)> = words
                .iter()
                .map(|(w, d)| (w.clone(), d.clone()))
                .collect();
            let word = word.clone();
            let definition = definition.clone();
            async move {
                build_choice_for_word(
                    state_ref,
                    &pool,
                    ecdict_pool.as_ref(),
                    &words_clone,
                    &word,
                    &definition,
                )
                .await
            }
        })
        .collect::<Vec<_>>();

    let results = futures::future::join_all(futures).await;
    let mut questions = Vec::new();
    for q in results {
        if let Some(q) = q {
            questions.push(q);
        }
    }

    // 如果本地卡牌不够，从 ECDICT 补充
    if questions.len() < count as usize {
        if let Some(ecdict_pool) = ecdict_pool {
            let extra: Vec<(String, String)> = sqlx::query_as(
                "SELECT word, translation FROM stardict WHERE frq IS NOT NULL ORDER BY RANDOM() LIMIT ?"
            )
            .bind(count - questions.len() as i32)
            .fetch_all(ecdict_pool)
            .await
            .unwrap_or_default();

            for (word, translation) in extra {
                let def_short = translation.lines().next().unwrap_or(&translation);
                let def_short: String = def_short.chars().take(40).collect();

                // T9: 语义近邻干扰项（本地卡牌池优先）
                let used: Vec<String> = vec![word.clone()];
                let distractors: Vec<String> =
                    pick_semantic_distractors(pool, ecdict_pool, &word, &used, 3).await;

                if distractors.len() < 3 {
                    // 语义向量不足时回退到 ECDICT 随机
                    let ecdict_distractors: Vec<String> = sqlx::query_scalar(
                        "SELECT word FROM stardict WHERE word != ? ORDER BY RANDOM() LIMIT 3",
                    )
                    .bind(&word)
                    .fetch_all(ecdict_pool)
                    .await
                    .unwrap_or_default();

                    if ecdict_distractors.len() < 3 {
                        continue;
                    }

                    let mut options = ecdict_distractors;
                    options.push(word.clone());
                    shuffle_vec(&mut options);
                    let correct_index = options.iter().position(|o| o == &word).unwrap_or(0);

                    questions.push(ChoiceQuestion {
                        word,
                        question: format!("哪个单词的意思是「{}」？", def_short),
                        options,
                        correct_index,
                        explanation: None,
                    });
                } else {
                    let mut options = distractors;
                    options.push(word.clone());
                    shuffle_vec(&mut options);
                    let correct_index = options.iter().position(|o| o == &word).unwrap_or(0);

                    questions.push(ChoiceQuestion {
                        word,
                        question: format!("哪个单词的意思是「{}」？", def_short),
                        options,
                        correct_index,
                        explanation: None,
                    });
                }
            }
        }
    }

    Ok(questions)
}

/// 为单个单词构建一道选择题（T9 语义干扰项 + LLM 歧义验证）
/// 返回 None 表示该题无法生成（干扰项不足），由调用方跳过
async fn build_choice_for_word(
    state: &State<'_, crate::AppState>,
    pool: &sqlx::SqlitePool,
    ecdict_pool: Option<&sqlx::SqlitePool>,
    words: &[(String, String)],
    word: &str,
    definition: &str,
) -> Option<ChoiceQuestion> {
    // 获取干扰选项（T9：语义近邻替代随机）
    let mut used: Vec<String> = words.iter().map(|(w, _)| w.clone()).collect();
    used.push(word.to_string());
    let distractors: Vec<String> = match ecdict_pool {
        Some(dict_pool) => pick_semantic_distractors(pool, dict_pool, word, &used, 3).await,
        None => Vec::new(),
    };

    if distractors.len() < 3 {
        // 语义向量不足时回退到随机（保留原有兜底）
        let fallback: Vec<String> =
            sqlx::query_scalar("SELECT word FROM cards WHERE word != ? ORDER BY RANDOM() LIMIT 3")
                .bind(word)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        if fallback.len() >= 3 {
            let mut options = fallback;
            options.push(word.to_string());
            shuffle_vec(&mut options);
            let correct_index = options.iter().position(|o| o == word).unwrap_or(0);
            return Some(ChoiceQuestion {
                word: word.to_string(),
                question: format!("哪个单词的意思是「{}」？", definition),
                options,
                correct_index,
                explanation: None,
            });
        }
        return None;
    }

    // 组装选项（正确答案 + 3个干扰项）
    // T9: AI 验证防幻觉——替换有歧义的干扰项
    let mut distractors = distractors;
    let ambiguous = verify_options_with_llm(state, word, definition, &distractors).await;
    if !ambiguous.is_empty() {
        let ambiguous_set: std::collections::HashSet<String> =
            ambiguous.into_iter().collect();
        distractors.retain(|d| !ambiguous_set.contains(d));

        // 补充替换词：用剩余候选补齐到 3 个（仅用语义近邻，不再 LLM 验证）
        if distractors.len() < 3 {
            let used: Vec<String> = {
                let mut u = words.iter().map(|(w, _)| w.clone()).collect::<Vec<_>>();
                u.push(word.to_string());
                u.extend(distractors.iter().cloned());
                u
            };
            let extra = match ecdict_pool {
                Some(dict_pool) => {
                    pick_semantic_distractors(pool, dict_pool, word, &used, 3).await
                },
                None => Vec::new(),
            };
            for e in extra {
                if distractors.len() >= 3 {
                    break;
                }
                if !distractors.contains(&e) {
                    distractors.push(e);
                }
            }
        }
    }

    // 契约兜底：LLM 删除歧义项且补充失败时，干扰项可能 < 3，跳过此题保持 4 选项
    if distractors.len() < 3 {
        return None;
    }

    let mut options = distractors;
    if !options.iter().any(|o| o == word) {
        options.push(word.to_string());
    }
    shuffle_vec(&mut options);
    let correct_index = options.iter().position(|o| o == word).unwrap_or(0);

    Some(ChoiceQuestion {
        word: word.to_string(),
        question: format!("哪个单词的意思是「{}」？", definition),
        options,
        correct_index,
        explanation: None,
    })
}

/// 生成拼写题
#[tauri::command]
pub async fn generate_spelling_questions(
    state: State<'_, crate::AppState>,
    plan_id: Option<String>,
    count: i32,
) -> Result<Vec<SpellingQuestion>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let words = get_quiz_words(pool, plan_id, count).await?;

    let mut questions = Vec::new();

    for (word, definition) in &words {
        let first_letter = word
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_default();
        let underscore_hint = first_letter.clone() + &"_".repeat(word.len() - 1);
        let hint = format!("{} ({}个字母)", underscore_hint, word.len());

        // 尝试获取例句
        let example: Option<String> = None;

        questions.push(SpellingQuestion {
            definition: definition.clone(),
            hint,
            answer: word.clone(),
            example,
        });
    }

    Ok(questions)
}

/// 生成填空题
#[tauri::command]
pub async fn generate_cloze_questions(
    state: State<'_, crate::AppState>,
    plan_id: Option<String>,
    count: i32,
) -> Result<Vec<ClozeQuestion>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let words = get_quiz_words(pool, plan_id, count).await?;

    let mut questions = Vec::new();

    for (word, definition) in &words {
        // 用释义构造简单填空句
        let sentence = format!("意思为「{}」的英文单词是：____", definition);

        // 获取干扰选项
        let distractors: Vec<String> =
            sqlx::query_scalar("SELECT word FROM cards WHERE word != ? ORDER BY RANDOM() LIMIT 3")
                .bind(word)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

        let mut options = distractors;
        options.push(word.clone());
        shuffle_vec(&mut options);

        questions.push(ClozeQuestion {
            sentence,
            answer: word.clone(),
            options,
            context: Some(definition.clone()),
        });
    }

    Ok(questions)
}

/// 获取复习模式的快速卡片栈（用于左右滑动模式）
#[tauri::command]
pub async fn get_swipe_cards(
    state: State<'_, crate::AppState>,
    count: i32,
) -> Result<Vec<crate::commands::vocabulary_cmd::CardInfo>, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let now = chrono::Utc::now().timestamp();

    let rows = sqlx::query(
        "SELECT id, word, fsrs_state FROM cards
         WHERE json_extract(fsrs_state, '$.next_review') <= ?
         ORDER BY json_extract(fsrs_state, '$.next_review') ASC
         LIMIT ?",
    )
    .bind(now)
    .bind(count)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let cards = rows
        .into_iter()
        .map(|row| {
            let fsrs_state_str: String = row.get("fsrs_state");
            let fsrs_state: serde_json::Value =
                serde_json::from_str(&fsrs_state_str).unwrap_or_default();

            crate::commands::vocabulary_cmd::CardInfo {
                id: row.get("id"),
                word: row.get("word"),
                phase: crate::domain::LearningPhase::Learning,
                next_review: fsrs_state["next_review"].as_i64().unwrap_or(0),
                reps: fsrs_state["reps"].as_u64().unwrap_or(0) as u32,
                stability: fsrs_state["stability"].as_f64().unwrap_or(0.0),
            }
        })
        .collect();

    Ok(cards)
}

/// 记录快速复习的评分
#[tauri::command]
pub async fn submit_swipe_rating(
    state: State<'_, crate::AppState>,
    card_id: String,
    rating: String,
) -> Result<(), String> {
    let rating_enum = match rating.to_lowercase().as_str() {
        "again" => crate::domain::Rating::Again,
        "hard" => crate::domain::Rating::Hard,
        "good" => crate::domain::Rating::Good,
        "easy" => crate::domain::Rating::Easy,
        _ => return Err(format!("无效评分: {}", rating)),
    };
    crate::commands::vocabulary_cmd::submit_review(state, card_id, rating_enum).await
}

// ============================================
// 辅助函数
// ============================================

/// 语义化选择干扰项（T9）
///
/// 从候选词池中挑选与目标词语义相近(余弦相似度 0.4-0.8)的 3 个词作干扰项,
/// 替代原来的 ORDER BY RANDOM() 完全随机。
/// 向量基于 ECDICT 释义文本构建,懒加载并缓存到 embeddings 表。
async fn pick_semantic_distractors(
    emb_pool: &sqlx::SqlitePool,
    dict_pool: &sqlx::SqlitePool,
    word: &str,
    exclude: &[String],
    count: usize,
) -> Vec<String> {
    // 候选词池：从 ECDICT 高频词取
    let mut candidates: Vec<(String, String)> = sqlx::query_as(
        "SELECT word, translation FROM stardict
         WHERE frq IS NOT NULL AND frq <= 6000 AND word != ?
         ORDER BY RANDOM() LIMIT 80",
    )
    .bind(word)
    .fetch_all(dict_pool)
    .await
    .unwrap_or_default();

    // 排除已用词
    candidates.retain(|(w, _)| !exclude.contains(w));

    if candidates.is_empty() {
        return Vec::new();
    }

    // 目标词释义（用于构建目标向量）
    let target_text: Option<String> = sqlx::query_scalar(
        "SELECT translation FROM stardict WHERE word = ?1",
    )
    .bind(word)
    .fetch_optional(dict_pool)
    .await
    .ok()
    .flatten();

    // 目标词向量（优先读缓存，未命中则从释义构建）
    let target_vec = match crate::infrastructure::load_embedding(emb_pool, word, "ecdict").await {
        Ok(Some(v)) => v,
        _ => {
            let text = target_text.clone().unwrap_or_else(|| word.to_string());
            let v = crate::infrastructure::build_vector(&text, 256);
            crate::infrastructure::upsert_embedding(emb_pool, word, "ecdict", &v).await.ok();
            v
        },
    };

    // 候选词向量（批量加载，未命中则从释义构建）
    // T9 优化：只对尚未缓存的候选构建向量（已缓存的直接复用，避免重复计算）
    let cand_words: Vec<String> = candidates.iter().map(|(w, _)| w.clone()).collect();
    let mut vec_map = crate::infrastructure::load_embeddings(emb_pool, &cand_words, "ecdict")
        .await
        .unwrap_or_default();

    for (w, text) in &candidates {
        if !vec_map.contains_key(w) {
            let v = crate::infrastructure::build_vector(text, 256);
            crate::infrastructure::upsert_embedding(emb_pool, w, "ecdict", &v).await.ok();
            vec_map.insert(w.clone(), v);
        }
    }

    // 计算相似度，选 0.4-0.8 区间内最接近的 count 个
    let mut scored: Vec<(f32, String)> = candidates
        .iter()
        .filter_map(|(w, _)| {
            let v = vec_map.get(w)?;
            let sim = target_vec.cosine(v);
            Some((sim, w.clone()))
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // 优先取 0.4-0.8 区间（语义相近但不易混淆），不足则退而取最接近的
    let in_range: Vec<String> = scored
        .iter()
        .filter(|(sim, _)| (*sim >= 0.4) && (*sim <= 0.8))
        .map(|(_, w)| w.clone())
        .take(count)
        .collect();

    if in_range.len() >= count {
        in_range
    } else {
        let mut result = in_range;
        for (_, w) in &scored {
            if result.len() >= count {
                break;
            }
            if !result.contains(w) {
                result.push(w.clone());
            }
        }
        result
    }
}

/// AI 词义验证防幻觉（T9）
///
/// 检查干扰项是否与正确答案语义冲突（多个选项都可能正确 = 歧义题）。
/// 返回需要替换的干扰项列表；无法调用 LLM 时返回空（不阻塞，静默跳过）。
/// 带缓存：相同 (正确词, 干扰项组合) 只验证一次。
async fn verify_options_with_llm(
    state: &State<'_, crate::AppState>,
    correct_word: &str,
    definition: &str,
    distractors: &[String],
) -> Vec<String> {
    if distractors.is_empty() {
        return Vec::new();
    }

    // LLM 配置
    let config = state.system.config.lock().await;
    let provider = crate::skills::llm_provider::provider_from_config(&config.llm);
    drop(config);

    let Some(provider) = provider else {
        return Vec::new();
    };

    let options_text = distractors
        .iter()
        .enumerate()
        .map(|(i, w)| format!("{}. {}", i + 1, w))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "你是一个单词学习题质量检查器。题目是「哪个单词的意思是「{}」？」，正确答案是「{}」。\n\n干扰选项：\n{}\n\n请判断：是否有某个干扰选项的意思是\"可以理解为 {} 或与之非常接近、会导致歧义\"？\n只返回 JSON，格式：\n{{\"ambiguous_indices\": [下标数组，1起（与上面列表的编号一致）], \"reason\": \"简述\"}}\n如果没有歧义，返回 {{\"ambiguous_indices\": [], \"reason\": \"\"}}\n只返回 JSON。",
        definition,
        correct_word,
        options_text,
        definition
    );

    let request = LlmRequest::new(vec![
        LlmMessage::system("你是严格、准确的单词学习题质量检查器。"),
        LlmMessage::user(prompt),
    ])
    .with_temperature(0.0)
    .with_max_tokens(300)
    .with_json_schema(serde_json::json!({ "type": "object" }));

    let response = match provider.complete(request).await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("LLM 选项验证跳过: {}", e);
            return Vec::new();
        },
    };

    let json_str = extract_json(&response.content);
    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let idxs: Vec<usize> = parsed
        .get("ambiguous_indices")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64())
                .filter_map(|u| u.checked_sub(1).map(|x| x as usize)) // LLM 返回 1-based（与选项列表编号一致），转为 0-based
                .collect()
        })
        .unwrap_or_default();

    idxs.iter()
        .filter_map(|i| distractors.get(*i).cloned())
        .collect()
}

/// 获取要测试的单词列表
async fn get_quiz_words(
    pool: &sqlx::SqlitePool,
    plan_id: Option<String>,
    count: i32,
) -> Result<Vec<(String, String)>, String> {
    let rows = if let Some(pid) = plan_id {
        // 从学习计划中取
        sqlx::query(
            "SELECT c.word, c.ai_content FROM plan_words pw
             JOIN cards c ON c.word = pw.word
             WHERE pw.plan_id = ? AND pw.learned = 1
             ORDER BY RANDOM() LIMIT ?",
        )
        .bind(&pid)
        .bind(count)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        // 从所有卡牌中取
        sqlx::query(
            "SELECT word, ai_content FROM cards
             WHERE ai_content IS NOT NULL
             ORDER BY RANDOM() LIMIT ?",
        )
        .bind(count)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let words: Vec<(String, String)> = rows
        .into_iter()
        .filter_map(|row| {
            let word: String = row.get("word");
            let ai_content_str: Option<String> = row.get("ai_content");

            // 从 AI 内容中提取简短释义
            let definition = ai_content_str
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| {
                    // 尝试从 mnemonics 中提取
                    v["mnemonics"].as_array().and_then(|arr| {
                        arr.first().and_then(|m| {
                            m["content"]
                                .as_str()
                                .map(|s| s.chars().take(40).collect::<String>())
                        })
                    })
                })
                .unwrap_or_else(|| format!("单词: {}", word));

            Some((word, definition))
        })
        .collect();

    Ok(words)
}

/// 打乱数组（Fisher-Yates 洗牌，使用数据库的 RANDOM() 作为随机源的补充）
fn shuffle_vec<T>(v: &mut Vec<T>) {
    let len = v.len();
    if len <= 1 {
        return;
    }
    // 使用时间戳 + 指针地址作为种子
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut state = seed.wrapping_add(0x9E3779B97F4A7C15);
    for i in (1..len).rev() {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state as usize) % (i + 1);
        v.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    /// 建测试所需的内存表：stardict（候选词池）+ embeddings（向量缓存）
    async fn test_pools() -> (SqlitePool, SqlitePool) {
        let dict_pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE stardict (
                word TEXT PRIMARY KEY,
                translation TEXT NOT NULL,
                frq INTEGER
            )",
        )
        .execute(&dict_pool)
        .await
        .unwrap();

        let emb_pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS embeddings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                word TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT 'ecdict',
                vector TEXT NOT NULL,
                dim INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE(word, source)
            )",
        )
        .execute(&emb_pool)
        .await
        .unwrap();

        (dict_pool, emb_pool)
    }

    #[tokio::test]
    async fn test_pick_semantic_distractors_returns_count_words() {
        let (dict_pool, emb_pool) = test_pools().await;

        // 目标词 + 5 个候选词（frq <= 6000 才进候选池）
        let words = [
            ("happy", "快乐的 高兴的 开心的 幸福", 100),
            ("glad", "高兴的 快乐的 愉快的 欢喜", 200),
            ("joyful", "快乐的 高兴的 欣喜的 欢快", 300),
            ("sad", "悲伤的 难过的 忧愁的 哀伤", 400),
            ("fast", "快的 迅速的 快速的 敏捷", 500),
            ("slow", "慢的 迟缓的 缓慢的 徐徐", 600),
        ];
        for (w, t, f) in words {
            sqlx::query("INSERT INTO stardict (word, translation, frq) VALUES (?, ?, ?)")
                .bind(w)
                .bind(t)
                .bind(f)
                .execute(&dict_pool)
                .await
                .unwrap();
        }

        let result = pick_semantic_distractors(&emb_pool, &dict_pool, "happy", &[], 3).await;

        assert_eq!(result.len(), 3, "应返回 3 个干扰项");
        assert!(!result.contains(&"happy".to_string()), "干扰项不能包含目标词");
        for w in &result {
            assert!(words.iter().any(|(word, _, _)| word == w), "干扰项必须来自候选池");
        }
    }

    #[tokio::test]
    async fn test_pick_semantic_distractors_excludes_used_words() {
        let (dict_pool, emb_pool) = test_pools().await;

        let words = [
            ("apple", "苹果 一种水果 清脆 甘甜", 100),
            ("pear", "梨 一种水果 多汁 香甜", 200),
            ("peach", "桃 一种水果 甜美 软糯", 300),
            ("car", "汽车 交通工具 引擎 速度", 400),
            ("bike", "自行车 交通工具 骑行 轮子", 500),
            ("bus", "公共汽车 交通工具 载客 路线", 600),
        ];
        for (w, t, f) in words {
            sqlx::query("INSERT INTO stardict (word, translation, frq) VALUES (?, ?, ?)")
                .bind(w)
                .bind(t)
                .bind(f)
                .execute(&dict_pool)
                .await
                .unwrap();
        }

        // 排除 pear（已被其他题使用）
        let exclude = vec!["pear".to_string()];
        let result = pick_semantic_distractors(&emb_pool, &dict_pool, "apple", &exclude, 3).await;

        assert_eq!(result.len(), 3, "应返回 3 个干扰项");
        assert!(
            !result.contains(&"pear".to_string()),
            "已被排除的词不应出现在干扰项中"
        );
    }

    #[tokio::test]
    async fn test_pick_semantic_distractors_empty_pool_returns_empty() {
        let (dict_pool, emb_pool) = test_pools().await;

        // 候选池为空（只有目标词，frq <= 6000 的候选中不含其他词）
        sqlx::query("INSERT INTO stardict (word, translation, frq) VALUES (?, ?, ?)")
            .bind("alone")
            .bind("独自的 孤单的 单独的 孑然")
            .bind(100)
            .execute(&dict_pool)
            .await
            .unwrap();

        let result = pick_semantic_distractors(&emb_pool, &dict_pool, "alone", &[], 3).await;
        assert!(result.is_empty(), "候选池不足时返回空");
    }
}

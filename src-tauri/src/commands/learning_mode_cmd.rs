// Learning Mode Commands - 多样化学习模式 API

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

    // 获取要测试的单词
    let words = get_quiz_words(pool, plan_id, count).await?;

    let mut questions = Vec::new();

    for word_info in &words {
        let (word, definition) = word_info;

        // 获取干扰选项（其他单词的释义）
        let distractors: Vec<String> =
            sqlx::query_scalar("SELECT word FROM cards WHERE word != ? ORDER BY RANDOM() LIMIT 3")
                .bind(word)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

        if distractors.len() < 3 {
            continue;
        }

        // 组装选项（正确答案 + 3个干扰项）
        let mut options = distractors;
        options.push(word.clone());
        shuffle_vec(&mut options);
        let correct_index = options.iter().position(|o| o == word).unwrap_or(0);

        questions.push(ChoiceQuestion {
            word: word.clone(),
            question: format!("哪个单词的意思是「{}」？", definition),
            options,
            correct_index,
            explanation: None,
        });
    }

    // 如果本地卡牌不够，从 ECDICT 补充
    if questions.len() < count as usize {
        let ecdict_pool = state.ecdict_pool.as_ref();
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

                let distractors: Vec<String> =
                    sqlx::query_scalar("SELECT word FROM cards ORDER BY RANDOM() LIMIT 3")
                        .fetch_all(pool)
                        .await
                        .unwrap_or_default();

                if distractors.len() < 3 {
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

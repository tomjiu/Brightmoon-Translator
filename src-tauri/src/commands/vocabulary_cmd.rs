// Vocabulary Commands - 词汇学习 API

use crate::commands::fsrs_optimization_cmd::build_fsrs_engine;
use crate::domain::{AiContent, LearningPhase, Rating, WordCard};
use crate::services::multi_dictionary::MultiSourceDictionary;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;

/// 卡牌信息（简化版，用于列表）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInfo {
    pub id: String,
    pub word: String,
    pub phase: LearningPhase,
    pub next_review: i64,
    pub reps: u32,
    pub stability: f64,
}

/// 核心词库词条
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreVocabEntry {
    pub word: String,
    pub frequency_rank: i64,
    pub frq: Option<i64>,
    pub collins: Option<i64>,
    pub oxford: Option<i64>,
    pub tag: Option<String>,
}

/// 获取核心词库列表
#[tauri::command]
pub async fn get_core_vocabulary(
    state: State<'_, crate::AppState>,
    offset: i64,
    limit: i64,
) -> Result<Vec<CoreVocabEntry>, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();

    let rows = sqlx::query(
        r"
        SELECT word, frequency_rank, frq, collins, oxford, tag
        FROM core_vocabulary
        ORDER BY frequency_rank
        LIMIT ? OFFSET ?
        ",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let entries = rows
        .into_iter()
        .map(|row| CoreVocabEntry {
            word: row.try_get("word").unwrap(),
            frequency_rank: row.try_get("frequency_rank").unwrap(),
            frq: row.try_get("frq").ok(),
            collins: row.try_get("collins").ok(),
            oxford: row.try_get("oxford").ok(),
            tag: row.try_get("tag").ok(),
        })
        .collect();

    Ok(entries)
}

/// 搜索核心词库
#[tauri::command]
pub async fn search_core_vocabulary(
    state: State<'_, crate::AppState>,
    query: String,
    limit: i64,
) -> Result<Vec<CoreVocabEntry>, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();
    let pattern = format!("%{query}%");

    let rows = sqlx::query(
        r"
        SELECT word, frequency_rank, frq, collins, oxford, tag
        FROM core_vocabulary
        WHERE word LIKE ?
        ORDER BY frequency_rank
        LIMIT ?
        ",
    )
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let entries = rows
        .into_iter()
        .map(|row| CoreVocabEntry {
            word: row.try_get("word").unwrap(),
            frequency_rank: row.try_get("frequency_rank").unwrap(),
            frq: row.try_get("frq").ok(),
            collins: row.try_get("collins").ok(),
            oxford: row.try_get("oxford").ok(),
            tag: row.try_get("tag").ok(),
        })
        .collect();

    Ok(entries)
}

/// 创建新卡牌
#[tauri::command]
pub async fn create_card(
    state: State<'_, crate::AppState>,
    word: String,
) -> Result<String, String> {
    use crate::domain::CardEvent;
    use chrono::Utc;
    use uuid::Uuid;

    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let card_id = Uuid::new_v4().to_string();

    let event = CardEvent::WordImported {
        word: word.clone(),
        source: "manual".to_string(),
        timestamp: Utc::now().timestamp(),
    };

    store
        .append_event(&card_id, &event)
        .await
        .map_err(|e| e.to_string())?;

    Ok(card_id)
}

/// 获取卡牌详情
#[tauri::command]
pub async fn get_card(
    state: State<'_, crate::AppState>,
    card_id: String,
) -> Result<WordCard, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    store
        .rebuild_card(&card_id)
        .await
        .map_err(|e| e.to_string())
}

/// 全文搜索卡牌(FTS5 + LIKE 兜底)— T5 接线:暴露 `EventStore::search_cards` 为命令
#[tauri::command]
pub async fn search_cards(
    state: State<'_, crate::AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<WordCard>, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let limit = limit.unwrap_or(20).clamp(1, 100);
    store
        .search_cards(&query, limit)
        .await
        .map_err(|e| e.to_string())
}

/// 获取待复习卡牌列表
#[tauri::command]
pub async fn get_due_cards(state: State<'_, crate::AppState>) -> Result<Vec<CardInfo>, String> {
    use chrono::Utc;

    let now = Utc::now().timestamp();
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();

    let rows = sqlx::query(
        r"
        SELECT id, word, fsrs_state, learning_state
        FROM cards
        WHERE json_extract(fsrs_state, '$.next_review') <= ?
        ORDER BY json_extract(fsrs_state, '$.next_review')
        LIMIT 100
        ",
    )
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut cards = Vec::new();
    for row in rows {
        let id: String = row.try_get("id").unwrap();
        let word: String = row.try_get("word").unwrap();
        let fsrs_json: String = row.try_get("fsrs_state").unwrap();
        let learning_json: Option<String> = row.try_get("learning_state").ok();

        let fsrs_state: crate::domain::CardState =
            serde_json::from_str(&fsrs_json).unwrap_or_default();
        let learning_state: crate::domain::LearningState = learning_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        cards.push(CardInfo {
            id,
            word,
            phase: learning_state.phase,
            next_review: fsrs_state.next_review,
            reps: fsrs_state.reps,
            stability: fsrs_state.stability,
        });
    }

    Ok(cards)
}

/// AI 生成卡牌内容
#[tauri::command]
pub async fn generate_card_content(
    state: State<'_, crate::AppState>,
    card_id: String,
) -> Result<AiContent, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let card = store
        .rebuild_card(&card_id)
        .await
        .map_err(|e| e.to_string())?;

    // 从配置获取 LLM 设置
    let config = state.system.config.lock().await;
    let provider =
        crate::skills::llm_provider::provider_from_config_for_learning(&config);
    drop(config);

    let provider = provider.ok_or("未配置 LLM API Key，请在设置中配置")?;

    use crate::skills::{GenerateCardSkill, SkillInput, SkillRegistry};

    let provider = std::sync::Arc::new(provider);

    let mut registry = SkillRegistry::new();
    registry
        .register(Box::new(GenerateCardSkill::new(provider)), 100)
        .map_err(|e| e.to_string())?;

    let context = serde_json::json!({
        "word": card.word,
        "definition": card.base_data.definitions.first().map(std::string::String::as_str),
        "translation": card.base_data.translation,
    });

    let input = SkillInput::new(&card.word).with_param("context", context);

    let output = registry
        .execute("generate_card", input)
        .await
        .map_err(|e| e.to_string())?;

    let model = output
        .metadata
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let ai_content: AiContent = output.into_type().map_err(|e| e.to_string())?;

    // 记录事件
    use crate::domain::CardEvent;
    use chrono::Utc;

    let event = CardEvent::AiContentGenerated {
        content: ai_content.clone(),
        model,
        confidence: 0.9,
        timestamp: Utc::now().timestamp(),
    };

    store
        .append_event(&card_id, &event)
        .await
        .map_err(|e| e.to_string())?;

    Ok(ai_content)
}

/// 掌握度等级(与学习方法论 weak/medium/strong 对应)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MasteryResult { Pass, Fail }

const MASTERY_WEAK: f64 = 0.0;
const MASTERY_MEDIUM: f64 = 1.0;
const MASTERY_STRONG: f64 = 2.0;

/// 按结果更新掌握度画像(存在 user_profile, field='mastery')
/// 规则: Fail → weak; 无画像 Pass → medium; weak Pass → medium;
///       medium Pass 且最近 2 次 quiz 全对 → strong
async fn update_mastery(pool: &sqlx::SqlitePool, card_id: &str, result: MasteryResult) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp();
    let current: Option<f64> = sqlx::query_scalar(
        "SELECT rating FROM user_profile WHERE card_id = ? AND field = 'mastery'",
    )
    .bind(card_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let next = match (current, result) {
        (_, MasteryResult::Fail) => MASTERY_WEAK,
        (None, MasteryResult::Pass) => MASTERY_MEDIUM,
        (Some(MASTERY_WEAK), MasteryResult::Pass) => MASTERY_MEDIUM,
        (Some(MASTERY_MEDIUM), MasteryResult::Pass) => {
            // 最近 2 次 quiz 全对 → 升 strong;否则保持 medium
            let last_two = sqlx::query(
                "SELECT user_answer, correct_answer FROM quiz_errors WHERE card_id = ? ORDER BY created_at DESC LIMIT 2",
            )
            .bind(card_id)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
            if last_two.len() == 2 && last_two.iter().all(|r| {
                let ua: String = r.try_get("user_answer").unwrap_or_default();
                let ca: String = r.try_get("correct_answer").unwrap_or_default();
                ua == ca
            }) {
                MASTERY_STRONG
            } else {
                MASTERY_MEDIUM
            }
        }
        (Some(level), MasteryResult::Pass) => level,
    };

    sqlx::query(
        r"
        INSERT INTO user_profile (card_id, field, rating, created_at, updated_at)
        VALUES (?, 'mastery', ?, ?, ?)
        ON CONFLICT (card_id, field) DO UPDATE SET
            rating = excluded.rating,
            updated_at = excluded.updated_at
        ",
    )
    .bind(card_id)
    .bind(next)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 读取某卡的掌握度画像(0/1/2, None=未记录)
async fn load_mastery_level(pool: &sqlx::SqlitePool, card_id: &str) -> Result<Option<u8>, String> {
    let rating: Option<f64> = sqlx::query_scalar(
        "SELECT rating FROM user_profile WHERE card_id = ? AND field = 'mastery'",
    )
    .bind(card_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rating.map(|r| r as u8))
}

/// 读取词典结果缓存(72 小时内有效,命中则跳过联网查询与 AI 生成)
async fn read_dictionary_cache(
    pool: &sqlx::SqlitePool,
    word: &str,
) -> Result<Option<StudyWordData>, String> {
    let row: Option<(String, i64)> = sqlx::query_as(
        "SELECT data, cached_at FROM dictionary_cache WHERE word = ?",
    )
    .bind(word)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((data, cached_at)) = row else {
        return Ok(None);
    };
    let now = chrono::Utc::now().timestamp();
    if now - cached_at > 72 * 3600 {
        return Ok(None);
    }
    serde_json::from_str::<StudyWordData>(&data)
        .map(Some)
        .map_err(|e| e.to_string())
}

/// 写入词典结果缓存(供 study_word 复用,避免重复联网与重复 AI 调用)
async fn write_dictionary_cache(
    pool: &sqlx::SqlitePool,
    word: &str,
    data: &StudyWordData,
) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp();
    let json = serde_json::to_string(data).map_err(|e| e.to_string())?;
    sqlx::query(
        r"
        INSERT INTO dictionary_cache (word, data, cached_at)
        VALUES (?, ?, ?)
        ON CONFLICT (word) DO UPDATE SET
            data = excluded.data,
            cached_at = excluded.cached_at
        ",
    )
    .bind(word)
    .bind(json)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 提交复习结果
#[tauri::command]
pub async fn submit_review(
    state: State<'_, crate::AppState>,
    card_id: String,
    rating: Rating,
) -> Result<(), String> {
    use crate::domain::CardEvent;
    use chrono::Utc;

    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let card = store
        .rebuild_card(&card_id)
        .await
        .map_err(|e| e.to_string())?;

    // 计算新的 FSRS 状态（T11: 优先使用已优化的持久化参数）
    let fsrs = build_fsrs_engine(store).await;
    let new_state = fsrs
        .schedule_review(&card.fsrs_state, rating, Utc::now())
        .map_err(|e| e.to_string())?;

    // 记录事件
    let event = CardEvent::FsrsUpdated {
        grade: rating,
        new_state,
        timestamp: Utc::now().timestamp(),
    };

    store
        .append_event(&card_id, &event)
        .await
        .map_err(|e| e.to_string())?;

    // 刷新卡牌快照,保证 get_due_cards 读到最新的 next_review
    if let Ok(updated) = store.rebuild_card(&card_id).await {
        store
            .update_snapshot(&updated)
            .await
            .map_err(|e| format!("更新快照失败: {e}"))?;
    }

    // 掌握度画像更新: Again/Hard → Fail, Good/Easy → Pass
    let mastery_result = match rating {
        Rating::Again | Rating::Hard => MasteryResult::Fail,
        Rating::Good | Rating::Easy => MasteryResult::Pass,
    };
    let _ = update_mastery(store.pool(), &card_id, mastery_result).await;

    Ok(())
}

/// 获取学习统计
#[tauri::command]
pub async fn get_learning_stats(
    state: State<'_, crate::AppState>,
) -> Result<LearningStats, String> {
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();
    let now = chrono::Utc::now().timestamp();
    let today_start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    let total_cards: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cards")
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    let due_cards: i64 = sqlx::query_scalar(
        r"
        SELECT COUNT(*) FROM cards
        WHERE json_extract(fsrs_state, '$.next_review') <= ?
        ",
    )
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let learned_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT card_id) FROM card_events
         WHERE event_type = 'word_imported' AND timestamp >= ?",
    )
    .bind(today_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let reviewed_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT card_id) FROM card_events
         WHERE event_type = 'fsrs_updated' AND timestamp >= ?",
    )
    .bind(today_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    Ok(LearningStats {
        total_cards: total_cards as u32,
        due_cards: due_cards as u32,
        learned_today: learned_today as u32,
        reviewed_today: reviewed_today as u32,
    })
}

/// 学习统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStats {
    pub total_cards: u32,
    pub due_cards: u32,
    pub learned_today: u32,
    pub reviewed_today: u32,
}

/// 学习单词的完整数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyWordData {
    pub word: String,
    pub card_id: Option<String>,
    pub phonetic: Option<String>,
    pub chinese_translation: Option<String>,
    pub english_definitions: Vec<String>,
    pub collins_entries: Vec<crate::commands::dictionary_cmd::CollinsEntry>,
    pub examples: Vec<crate::commands::dictionary_cmd::BilingualExample>,
    pub us_audio_url: Option<String>,
    pub uk_audio_url: Option<String>,
    pub ai_content: Option<crate::domain::AiContent>,
    pub image_url: Option<String>,
    pub sources: Vec<String>,
    pub mastery_level: Option<u8>,
}

/// 学习一个单词：查词典 + 创建卡牌 + 生成 AI 内容
#[tauri::command]
pub async fn study_word(
    state: State<'_, crate::AppState>,
    word: String,
) -> Result<StudyWordData, String> {
    // 词典数据由 study_word 命令处理
    // P1 修复:word 归一化(trim),大小写归一化通过 COLLATE NOCASE 查询实现
    let word = word.trim().to_string();
    if word.is_empty() {
        return Err("word is empty".to_string());
    }

    let ecdict_pool = state.ecdict_pool.as_ref();
    let event_store = state.event_store.as_ref();

    // 0. 命中本地缓存则直接返回(72h 内,避免重复联网与重复 AI 调用)
    if let Some(store) = event_store {
        if let Ok(Some(mut cached)) = read_dictionary_cache(store.pool(), &word).await {
            // 掌握度是动态值,不随缓存固化,实时读取
            cached.mastery_level = if let Some(cid) = cached.card_id.clone() {
                load_mastery_level(store.pool(), &cid).await.ok().flatten()
            } else {
                None
            };
            return Ok(cached);
        }
    }

    // 1. 查词典（多源聚合）
    let dict = MultiSourceDictionary::new();

    // 并行查 ECDICT + 有道 + DictionaryAPI + 图片
    let (ecdict_result, youdao_result, online_result, image_url) = tokio::join!(
        async {
            match ecdict_pool {
                Some(p) => {
                    let mut r = lookup_ecdict_simple(&word, p).await;
                    if r.is_err() {
                        // 变形词（ran → run）未命中时，用 lemma 还原后重查
                        if let Ok(Some(lemma)) = crate::commands::dictionary_cmd::resolve_lemma(&word, p).await
                        {
                            if lemma != word {
                                r = lookup_ecdict_simple(&lemma, p).await;
                            }
                        }
                    }
                    r
                }
                None => Err("no pool".into()),
            }
        },
        async { parse_youdao_simple(&dict, &word).await },
        async {
            dict.lookup(&word).await.ok().and_then(|mut v| {
                if v.is_empty() {
                    None
                } else {
                    Some(v.remove(0))
                }
            })
        },
        async { fetch_word_image(&word).await }
    );

    // 2. 查本地 Oxford/GPT4
    let (oxford_def, _gpt_analysis) = if let Some(store) = event_store {
        let pool = store.pool();
        let o = crate::commands::dictionary_cmd::lookup_oxford(&word, pool)
            .await
            .ok();
        let g = crate::commands::dictionary_cmd::lookup_gpt_dict(&word, pool)
            .await
            .ok();
        (o, g)
    } else {
        (None, None)
    };

    // 3. 合并结果
    let mut data = StudyWordData {
        word: word.clone(),
        card_id: None,
        phonetic: None,
        chinese_translation: None,
        english_definitions: Vec::new(),
        collins_entries: Vec::new(),
        examples: Vec::new(),
        us_audio_url: Some(format!(
            "https://dict.youdao.com/dictvoice?audio={}&type=2",
            urlencoding::encode(&word)
        )),
        uk_audio_url: Some(format!(
            "https://dict.youdao.com/dictvoice?audio={}&type=1",
            urlencoding::encode(&word)
        )),
        ai_content: None,
        image_url,
        sources: Vec::new(),
        mastery_level: None,
    };

    // ECDICT
    if let Ok((zh, en, phonetic)) = &ecdict_result {
        data.chinese_translation = zh.clone();
        data.english_definitions = en.clone();
        data.phonetic = phonetic.clone();
        data.sources.push("ECDICT".into());
    }

    // 有道
    if let Some(youdao) = youdao_result {
        if data.phonetic.is_none() {
            data.phonetic = youdao.phonetic;
        }
        if data.chinese_translation.is_none() && !youdao.chinese_translations.is_empty() {
            data.chinese_translation = Some(youdao.chinese_translations.join("；"));
        }
        data.examples = youdao.examples;
        data.collins_entries = youdao.collins_entries;
        if !data.collins_entries.is_empty() {
            data.sources.push("柯林斯".into());
        }
        data.sources.push("有道".into());
    }

    // Oxford
    if let Some(def) = oxford_def {
        data.sources.push("Oxford".into());
        // 可以附加到 english_definitions
        if data.english_definitions.is_empty() {
            data.english_definitions.push(def);
        }
    }

    // DictionaryAPI
    if let Some(online) = online_result {
        for m in &online.meanings {
            for d in &m.definitions {
                if !d.definition.is_empty() {
                    data.english_definitions.push(d.definition.clone());
                }
            }
        }
        data.sources.push("DictionaryAPI".into());
    }

    // 4. 创建/获取卡牌（Event Sourcing）
    if let Some(store) = event_store {
        use crate::domain::CardEvent;
        use uuid::Uuid;

        // 复用已存在的卡牌,避免同一单词生成多条 UUID / 重复 AI 调用
        // P1 修复:用 word_lower 查询,大小写归一化("Run" == "run")
        let pool = store.pool();
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM cards WHERE word = ?1 COLLATE NOCASE LIMIT 1")
                .bind(&word)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

        let is_new = existing.is_none();
        let card_id = match existing {
            Some(id) => id,
            None => Uuid::new_v4().to_string(),
        };
        let now = chrono::Utc::now().timestamp();

        // 已存在的卡牌直接复用,不重复生成 AI 内容
        if !is_new {
            data.card_id = Some(card_id.clone());
        }

        if is_new {
            let event = CardEvent::WordImported {
                word: word.clone(),
                source: "study".to_string(),
                timestamp: now,
            };

            match store.append_event(&card_id, &event).await {
                Ok(_) => {
                    data.card_id = Some(card_id.clone());

                    // 5. AI 生成内容
                    let config = state.system.config.lock().await;
                    let provider =
                        crate::skills::llm_provider::provider_from_config_for_learning(&config);
                    drop(config);

                    tracing::info!(
                        "AI gen check: key_set={}",
                        provider.is_some()
                    );

                    if let Some(provider) = provider {
                        let model_for_event = provider.model_name();
                        use crate::skills::{GenerateCardSkill, SkillInput, SkillRegistry};

                        let provider = std::sync::Arc::new(provider);
                        let mut registry = SkillRegistry::new();
                        if registry
                            .register(Box::new(GenerateCardSkill::new(provider)), 100)
                            .is_ok()
                        {
                                let context = serde_json::json!({
                                    "word": data.word,
                                    "translation": data.chinese_translation,
                                    "definitions": data.english_definitions,
                                });
                                let input = SkillInput::new(&data.word).with_param("context", context);

                                match registry.execute("generate_card", input).await {
                                    Ok(output) => {
                                        match output.into_type::<crate::domain::AiContent>() {
                                            Ok(ai_content) => {
                                                // 保存到事件流
                                                let event = CardEvent::AiContentGenerated {
                                                    content: ai_content.clone(),
                                                    model: model_for_event,
                                                    confidence: 0.9,
                                                    timestamp: now,
                                                };
                                                store
                                                    .append_event(&card_id, &event)
                                                    .await
                                                    .map_err(|e| {
                                                        tracing::warn!("保存AI内容事件失败: {}", e);
                                                    })
                                                    .ok();
                                                data.ai_content = Some(ai_content);
                                                data.sources.push("AI".into());
                                            },
                                            Err(e) => tracing::warn!("AI content parse failed: {}", e),
                                        }
                                    },
                                    Err(e) => {
                                        tracing::warn!("AI generation failed for '{}': {}", word, e);
                                    },
                                }
                            }
                        }

                    // 6. 更新卡牌快照
                    if let Ok(card) = store.rebuild_card(&card_id).await {
                        store
                            .update_snapshot(&card)
                            .await
                            .map_err(|e| tracing::warn!("更新卡牌快照失败: {}", e))
                            .ok();
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to create card for '{}': {}", word, e);
                },
            }
        }
    }

    // 附加掌握度画像(有卡牌时)
    if let Some(cid) = data.card_id.clone() {
        if let Some(store) = event_store {
            data.mastery_level = load_mastery_level(store.pool(), &cid).await.ok().flatten();
        }
    }

    // 写词典缓存(仅联网获取到数据时,避免缓存失败的空结果)
    if let Some(store) = event_store {
        if !data.sources.is_empty() {
            let _ = write_dictionary_cache(store.pool(), &word, &data).await;
        }
    }

    Ok(data)
}

/// 查询 ECDICT（简化版）
async fn lookup_ecdict_simple(
    word: &str,
    pool: &sqlx::SqlitePool,
) -> Result<(Option<String>, Vec<String>, Option<String>), String> {
    let row = sqlx::query("SELECT phonetic, definition, translation FROM stardict WHERE word = ?1")
        .bind(word)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

    if let Some(row) = row {
        let phonetic: Option<String> = row.get("phonetic");
        let definition: Option<String> = row.get("definition");
        let translation: Option<String> = row.get("translation");

        let clean_phonetic = phonetic.map(|p| {
            p.trim()
                .trim_start_matches('/')
                .trim_end_matches('/')
                .to_string()
        });

        let english: Vec<String> = definition
            .as_deref()
            .map(|d| {
                d.split('\n')
                    .filter(|s| !s.trim().is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        Ok((translation, english, clean_phonetic))
    } else {
        Err("not found".into())
    }
}

/// 解析有道（简化版）
async fn parse_youdao_simple(dict: &MultiSourceDictionary, word: &str) -> Option<YoudaoSimple> {
    let raw = dict.lookup_youdao(word).await.ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;

    let mut result = YoudaoSimple {
        chinese_translations: Vec::new(),
        phonetic: None,
        examples: Vec::new(),
        collins_entries: Vec::new(),
    };

    // 音标
    if let Some(wl) = json
        .get("ec")
        .and_then(|v| v.get("word"))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
    {
        result.phonetic = wl
            .get("usphone")
            .or_else(|| wl.get("ukphone"))
            .and_then(|v| v.as_str())
            .map(String::from);
    }

    // 中文释义
    if let Some(trs) = json
        .get("ec")
        .and_then(|v| v.get("word"))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.get("trs"))
        .and_then(|v| v.as_array())
    {
        for tr in trs {
            if let Some(txt) = tr
                .get("tr")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.get("l"))
                .and_then(|v| v.get("i"))
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
            {
                result.chinese_translations.push(txt.to_string());
            }
        }
    }

    // 柯林斯
    if let Some(collins) = json.get("collins") {
        if let Some(entries) = collins
            .get("collins_entries")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
        {
            if let Some(entry_arr) = entries
                .get("entries")
                .and_then(|v| v.get("entry"))
                .and_then(|v| v.as_array())
            {
                for entry in entry_arr {
                    if let Some(tran_entries) = entry.get("tran_entry").and_then(|v| v.as_array()) {
                        for te in tran_entries {
                            let pos = te
                                .get("pos_entry")
                                .and_then(|v| v.get("pos"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let pos_cn = te
                                .get("pos_entry")
                                .and_then(|v| v.get("pos_tips"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let en_def = te
                                .get("tran")
                                .and_then(|v| v.as_str())
                                .map(|s| {
                                    s.replace("<b>", "")
                                        .replace("</b>", "")
                                        .replace("<em>", "")
                                        .replace("</em>", "")
                                        .trim()
                                        .to_string()
                                })
                                .unwrap_or_default();

                            let mut collins_examples = Vec::new();
                            if let Some(sents) = te
                                .get("exam_sents")
                                .and_then(|v| v.get("sent"))
                                .and_then(|v| v.as_array())
                            {
                                for sent in sents.iter().take(2) {
                                    let en = sent
                                        .get("eng_sent")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let zh = sent
                                        .get("chn_sent")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    if !en.is_empty() {
                                        collins_examples.push(
                                            crate::commands::dictionary_cmd::BilingualExample {
                                                en,
                                                zh,
                                            },
                                        );
                                    }
                                }
                            }

                            if !en_def.is_empty() {
                                result.collins_entries.push(
                                    crate::commands::dictionary_cmd::CollinsEntry {
                                        pos,
                                        pos_cn,
                                        english_def: en_def,
                                        examples: collins_examples,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // 例句
    if let Some(sents) = json
        .get("blng_sents_part")
        .and_then(|v| v.get("sentence-pair"))
        .and_then(|v| v.as_array())
    {
        for pair in sents.iter().take(3) {
            let en = pair
                .get("sentence")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let zh = pair
                .get("sentence-translation")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !en.is_empty() {
                result
                    .examples
                    .push(crate::commands::dictionary_cmd::BilingualExample { en, zh });
            }
        }
    }

    if result.chinese_translations.is_empty()
        && result.examples.is_empty()
        && result.collins_entries.is_empty()
    {
        return None;
    }

    Some(result)
}

struct YoudaoSimple {
    chinese_translations: Vec<String>,
    phonetic: Option<String>,
    examples: Vec<crate::commands::dictionary_cmd::BilingualExample>,
    collins_entries: Vec<crate::commands::dictionary_cmd::CollinsEntry>,
}

/// 搜索单词相关的图片(Wikimedia Commons → 确定性 known 映射兜底)
async fn fetch_word_image(word: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    // 1. 优先: 确定性 known 映射(31 个常见单词,真实相关)
    let known_id = word_to_photo_id(word);
    if known_id != GENERIC_PHOTO_PLACEHOLDER {
        let direct = format!(
            "https://images.unsplash.com/photo-{}?w=800&h=500&fit=crop&auto=format",
            known_id
        );
        if let Ok(r) = client.head(&direct).send().await {
            if r.status().is_success() {
                return Some(direct);
            }
        }
    }

    // 2. Wikimedia Commons 免费图片搜索(无需 key,返回与单词相关的真实图片)
    let search = format!("{} vocabulary", word);
    let api = format!(
        "https://commons.wikimedia.org/w/api.php?action=query&generator=search&gsrsearch={}&gsrlimit=3&gsrnamespace=6&prop=imageinfo&iiprop=url&iiurlwidth=600&format=json",
        urlencoding::encode(&search)
    );
    let resp = client
        .get(&api)
        .header("User-Agent", "MoonTranslator/0.3 (vocabulary app)")
        .send()
        .await
        .ok()?;
    if resp.status().is_success() {
        if let Ok(v) = resp.json::<serde_json::Value>().await {
            if let Some(pages) = v["query"]["pages"].as_object() {
                for page in pages.values() {
                    if let Some(thumb) = page["imageinfo"].as_array().and_then(|a| a.first())
                        .and_then(|i| i["thumburl"].as_str())
                    {
                        return Some(thumb.to_string());
                    }
                }
            }
        }
    }

    None // 不可用时前端隐藏图片,不显示错位图
}

const GENERIC_PHOTO_PLACEHOLDER: &str = "__none__";

/// 将单词映射到 Unsplash photo ID（确定性，同一单词总是同一张图）
fn word_to_photo_id(word: &str) -> String {
    // 常见单词映射到高质量图片
    let known: &[(&str, &str)] = &[
        ("abandon", "1504280392369-61ec28e4e076"),
        ("beautiful", "1506744038136-46273834b3fb"),
        ("ocean", "1505118380757-91f5f5632de0"),
        ("mountain", "1464822759023-fed622ff2c3b"),
        ("forest", "1448375240586-882707db888b"),
        ("city", "1477959858617-67f85cf4f1df"),
        ("love", "1518199265813-4775cfe4d97e"),
        ("time", "1501139083538-0139583c060f"),
        ("light", "1507003211169-0a1dd7228f2d"),
        ("music", "1511671782779-c97d3d27a1d4"),
        ("book", "1512820790803-83ca734da794"),
        ("water", "1470071459604-3b5ec3a7fe05"),
        ("fire", "1517022812-69ee30f5b9f9"),
        ("star", "1419242902214-272b3f66ee7a"),
        ("dream", "1518837695005-2083093ee35b"),
        ("flower", "1490750967868-88aa4f44baee"),
        ("sun", "1506318137071-a8e063b4bec0"),
        ("moon", "1532693303140-7b80a9939e01"),
        ("heart", "1518199265813-4775cfe4d97e"),
        ("home", "1513694203232-719a280e022f"),
        ("tree", "1513836279014-a89f7a76ae86"),
        ("bird", "1522926193341-e9ffd686c60f"),
        ("cat", "1514888286974-6c03e2ca1dba"),
        ("dog", "1587300003388-59208cc962cb"),
        ("food", "1504674900247-0877df9cc836"),
        ("rain", "1515694346937-94d85e39d72e"),
        ("snow", "1491002052546-bf38f186af56"),
        ("wind", "1506905925346-21bda4d32df4"),
        ("earth", "1451187580459-43490279c0fa"),
        ("sky", "1517483000871-1dbf64a6e1c6"),
        ("sea", "1505118380757-91f5f5632de0"),
    ];

    for (w, id) in known {
        if w == &word.to_lowercase() {
            return id.to_string();
        }
    }

    // 未匹配: 交给 Wikimedia Commons 搜索,无需哈希兜底
    GENERIC_PHOTO_PLACEHOLDER.to_string()
}

/// 划词文本 AI 抽生词建本结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractStudyResult {
    pub total_words: usize,
    pub studied: Vec<StudyWordData>,
    pub skipped_existing: Vec<String>,
}

/// 从一段文本中提取生词并批量建卡（划词 AI 抽生词建本）
///
/// 1. 正则提取文本中的英文单词（去停用词、去标点、小写化）
/// 2. 过滤掉已在词库中的卡（cards.word）
/// 3. 用 ECDICT 词典校验词条存在，过滤太长的复合串
/// 4. 并行 `study_word` 建卡 + 生成 AI 内容
#[tauri::command]
pub async fn extract_words_and_study(
    state: State<'_, crate::AppState>,
    text: String,
) -> Result<ExtractStudyResult, String> {
    use std::collections::HashSet;

    let ecdict_pool = state.ecdict_pool.as_ref().ok_or("本地词典未连接")?;

    // 1. 提取英文单词
    let mut words: Vec<String> = text
        .split(|c: char| !c.is_ascii_alphabetic() && c != '\'' && c != '-')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().trim_matches(['\'', '-']).to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    // 停用词过滤（常见功能词 + 无学习价值的词）— P1 修复:去重
    let stop_words: HashSet<&str> = [
        "the", "a", "an", "and", "or", "but", "if", "then", "than", "so", "of", "to", "in",
        "on", "at", "by", "for", "with", "about", "as", "is", "are", "was", "were", "be",
        "been", "being", "am", "do", "does", "did", "done", "have", "has", "had", "having",
        "it", "its", "this", "that", "these", "those", "there", "here", "i", "you", "he",
        "she", "they", "we", "my", "your", "his", "her", "their", "our", "not", "no", "yes",
        "will", "would", "can", "could", "should", "shall", "may", "might", "must", "from",
        "into", "up", "down", "out", "off", "over", "under", "again", "once", "just", "very",
        "also", "which", "what", "when", "where", "why", "how", "who", "whom", "whose",
        "all", "any", "some", "each", "every", "both", "few", "more", "most", "other", "such",
        "only", "own", "same", "too", "them", "us", "me", "him",
        "don't", "doesn't", "didn't", "won't", "can't", "isn't", "aren't", "wasn't",
        "weren't", "i'm", "i've", "i'll", "i'd", "you're", "you've", "we're", "they're",
        "there's", "it's", "that's", "what's", "let's", "ok", "okay", "good", "well",
    ]
    .into_iter()
    .collect();

    words.retain(|w| !stop_words.contains(w.as_str()) && w.len() >= 2 && w.len() <= 20);

    // 去重（保序）
    let mut seen = HashSet::new();
    words.retain(|w| seen.insert(w.clone()));
    if words.is_empty() {
        return Ok(ExtractStudyResult {
            total_words: 0,
            studied: Vec::new(),
            skipped_existing: Vec::new(),
        });
    }

    // 2. 过滤已存在的卡
    let event_store = state.event_store.as_ref();
    let mut skipped_existing = Vec::new();
    if let Some(store) = event_store {
        let pool = store.pool();
        let mut existing = HashSet::new();
        let rows = sqlx::query("SELECT word FROM cards")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
        for row in rows {
            let w: String = row.try_get("word").unwrap_or_default();
            existing.insert(w.to_lowercase());
        }
        words.retain(|w| {
            if existing.contains(w) {
                skipped_existing.push(w.clone());
                false
            } else {
                true
            }
        });
    }

    // 3. ECDICT 校验词条存在（批量 IN 查询）
    if words.len() > 200 {
        words.truncate(200);
    }
    if !words.is_empty() {
        let placeholders = vec!["?"; words.len()].join(",");
        let sql = format!("SELECT word FROM stardict WHERE word IN ({placeholders})");
        let mut q = sqlx::query(&sql);
        for w in &words {
            q = q.bind(w);
        }
        let rows = q.fetch_all(ecdict_pool).await.map_err(|e| e.to_string())?;
        let known: HashSet<String> = rows
            .into_iter()
            .map(|r| r.try_get::<String, _>("word").unwrap_or_default())
            .collect();
        words.retain(|w| known.contains(w.as_str()));
    }

    let total_words = words.len();
    let mut studied = Vec::new();
    // 顺序建卡：AI 内容生成受 API 限流约束，串行比并发更稳
    for w in words {
        match study_word(state.clone(), w).await {
            Ok(data) => studied.push(data),
            Err(e) => tracing::warn!("study_word 建卡失败: {}", e),
        }
    }

    Ok(ExtractStudyResult {
        total_words,
        studied,
        skipped_existing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    /// 建内存库基础表(与 event_store.rs schema 保持一致的精简版)
    async fn base_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE cards (
                id TEXT PRIMARY KEY,
                word TEXT NOT NULL,
                current_version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "CREATE TABLE user_profile (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id TEXT NOT NULL,
                field TEXT NOT NULL,
                rating REAL DEFAULT 0,
                feedback TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(card_id, field)
            )",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "CREATE TABLE quiz_errors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id TEXT NOT NULL,
                quiz_type TEXT NOT NULL,
                user_answer TEXT,
                correct_answer TEXT,
                created_at INTEGER NOT NULL
            )",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "CREATE TABLE dictionary_cache (
                word TEXT PRIMARY KEY,
                payload TEXT,
                created_at INTEGER
            )",
        ).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn update_mastery_promotes_on_repeated_success() {
        let pool = base_pool().await;
        sqlx::query("INSERT INTO cards (id, word, current_version, created_at, updated_at) VALUES ('c1', 'hello', 1, 0, 0)")
            .execute(&pool).await.unwrap();

        update_mastery(&pool, "c1", MasteryResult::Pass).await.unwrap();
        update_mastery(&pool, "c1", MasteryResult::Pass).await.unwrap();

        let level: f64 = sqlx::query_scalar("SELECT rating FROM user_profile WHERE card_id = 'c1' AND field = 'mastery'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(level, 1.0, "两次通过应从 weak(0) 升到 medium(1)");
    }

    #[tokio::test]
    async fn update_mastery_fail_downgrades_to_weak() {
        let pool = base_pool().await;
        sqlx::query("INSERT INTO cards (id, word, current_version, created_at, updated_at) VALUES ('c1', 'hello', 1, 0, 0)")
            .execute(&pool).await.unwrap();
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO user_profile (card_id, field, rating, created_at, updated_at) VALUES ('c1', 'mastery', 1, ?, ?)")
            .bind(now).bind(now).execute(&pool).await.unwrap();

        update_mastery(&pool, "c1", MasteryResult::Fail).await.unwrap();

        let level: f64 = sqlx::query_scalar("SELECT rating FROM user_profile WHERE card_id = 'c1' AND field = 'mastery'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(level, 0.0, "Fail 应降到 weak(0)");
    }

    #[tokio::test]
    async fn update_mastery_two_consecutive_passes_upgrade_to_strong() {
        let pool = base_pool().await;
        sqlx::query("INSERT INTO cards (id, word, current_version, created_at, updated_at) VALUES ('c1', 'hello', 1, 0, 0)")
            .execute(&pool).await.unwrap();
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO user_profile (card_id, field, rating, created_at, updated_at) VALUES ('c1', 'mastery', 1, ?, ?)")
            .bind(now).bind(now).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO quiz_errors (card_id, quiz_type, user_answer, correct_answer, created_at) VALUES ('c1', 'choice', 'apple', 'apple', ?), ('c1', 'choice', 'apple', 'apple', ?)")
            .bind(now).bind(now + 1).execute(&pool).await.unwrap();

        update_mastery(&pool, "c1", MasteryResult::Pass).await.unwrap();

        let level: f64 = sqlx::query_scalar("SELECT rating FROM user_profile WHERE card_id = 'c1' AND field = 'mastery'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(level, 2.0, "连续两次答对应升级到 strong(2)");
    }

    #[tokio::test]
    async fn fetch_word_image_returns_relevant_image() {
        let url = fetch_word_image("apple").await;
        assert!(url.is_some(), "apple 应能获取到相关图片");
        if let Some(u) = url {
            assert!(
                u.contains("wikimedia.org") || u.contains("unsplash.com"),
                "图片应来自 Wikimedia/Unsplash: {u}"
            );
        }
    }
}

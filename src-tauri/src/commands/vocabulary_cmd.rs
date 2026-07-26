// Vocabulary Commands - 词汇学习 API

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
        r#"
        SELECT word, frequency_rank, frq, collins, oxford, tag
        FROM core_vocabulary
        ORDER BY frequency_rank
        LIMIT ? OFFSET ?
        "#,
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
    let pattern = format!("%{}%", query);

    let rows = sqlx::query(
        r#"
        SELECT word, frequency_rank, frq, collins, oxford, tag
        FROM core_vocabulary
        WHERE word LIKE ?
        ORDER BY frequency_rank
        LIMIT ?
        "#,
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

/// 获取待复习卡牌列表
#[tauri::command]
pub async fn get_due_cards(state: State<'_, crate::AppState>) -> Result<Vec<CardInfo>, String> {
    use chrono::Utc;

    let now = Utc::now().timestamp();
    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let pool = store.pool();

    let rows = sqlx::query(
        r#"
        SELECT id, word, fsrs_state, learning_state
        FROM cards
        WHERE json_extract(fsrs_state, '$.next_review') <= ?
        ORDER BY json_extract(fsrs_state, '$.next_review')
        LIMIT 100
        "#,
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
    let llm_config = &config.llm;
    let api_key = if !llm_config.api_key.is_empty() {
        llm_config.api_key.clone()
    } else if let Some(key) = llm_config.api_keys.first() {
        key.clone()
    } else {
        return Err("未配置 LLM API Key，请在设置中配置".to_string());
    };
    let base_url = llm_config.base_url.clone();
    let model = llm_config.model.clone();
    drop(config);

    use crate::skills::{GenerateCardSkill, OpenAiCompatibleProvider, SkillInput, SkillRegistry};

    let provider = std::sync::Arc::new(OpenAiCompatibleProvider::new(api_key, base_url, model));

    let mut registry = SkillRegistry::new();
    registry
        .register(Box::new(GenerateCardSkill::new(provider)), 100)
        .map_err(|e| e.to_string())?;

    let context = serde_json::json!({
        "word": card.word,
        "definition": card.base_data.definitions.first().map(|d| d.as_str()),
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

/// 提交复习结果
#[tauri::command]
pub async fn submit_review(
    state: State<'_, crate::AppState>,
    card_id: String,
    rating: Rating,
) -> Result<(), String> {
    use crate::domain::{CardEvent, FsrsEngine};
    use chrono::Utc;

    let store = state.event_store.as_ref().ok_or("词汇数据库未初始化")?;
    let card = store
        .rebuild_card(&card_id)
        .await
        .map_err(|e| e.to_string())?;

    // 计算新的 FSRS 状态
    let fsrs = FsrsEngine::new();
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
        r#"
        SELECT COUNT(*) FROM cards
        WHERE json_extract(fsrs_state, '$.next_review') <= ?
        "#,
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
}

/// 学习一个单词：查词典 + 创建卡牌 + 生成 AI 内容
#[tauri::command]
pub async fn study_word(
    state: State<'_, crate::AppState>,
    word: String,
) -> Result<StudyWordData, String> {
    // 词典数据由 study_word 命令处理

    let ecdict_pool = state.ecdict_pool.as_ref();
    let event_store = state.event_store.as_ref();

    // 1. 查词典（多源聚合）
    let dict = MultiSourceDictionary::new();

    // 并行查 ECDICT + 有道 + DictionaryAPI + 图片
    let (ecdict_result, youdao_result, online_result, image_url) = tokio::join!(
        async {
            match ecdict_pool {
                Some(p) => lookup_ecdict_simple(&word, p).await,
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

        let card_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

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
                let llm = &config.llm;
                let api_key = if !llm.api_key.is_empty() {
                    Some(llm.api_key.clone())
                } else {
                    llm.api_keys.first().cloned()
                };
                let base_url = llm.base_url.clone();
                let model = llm.model.clone();
                drop(config);

                tracing::info!(
                    "AI gen check: key_set={}, base_url='{}', model='{}'",
                    api_key.is_some(),
                    base_url,
                    model
                );

                if let Some(key) = api_key {
                    if !key.is_empty() && !base_url.is_empty() {
                        let model_for_event = model.clone();
                        use crate::skills::{
                            GenerateCardSkill, OpenAiCompatibleProvider, SkillInput, SkillRegistry,
                        };

                        let provider = std::sync::Arc::new(OpenAiCompatibleProvider::new(
                            key, base_url, model,
                        ));
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
                                                    tracing::warn!("保存AI内容事件失败: {}", e)
                                                })
                                                .ok();
                                            data.ai_content = Some(ai_content);
                                            data.sources.push("AI".into());
                                        },
                                        Err(e) => tracing::warn!("AI content parse failed: {}", e),
                                    }
                                },
                                Err(e) => {
                                    tracing::warn!("AI generation failed for '{}': {}", word, e)
                                },
                            }
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

/// 搜索单词相关的图片（Unsplash → Pexels fallback）
async fn fetch_word_image(word: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    // 1. 尝试 Unsplash Source（免费，无需 API Key）
    let unsplash_url = format!(
        "https://source.unsplash.com/800x500/?{}",
        urlencoding::encode(word)
    );
    let resp = client
        .get(&unsplash_url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await;

    if let Ok(r) = resp {
        if r.status().is_success() || r.status().is_redirection() {
            // Unsplash source returns a redirect to the actual image
            let final_url = r.url().to_string();
            if final_url.contains("images.unsplash.com") {
                return Some(final_url);
            }
            // If it redirected, the final URL is the image
            if let Some(location) = r.headers().get("location") {
                if let Ok(loc) = location.to_str() {
                    return Some(loc.to_string());
                }
            }
        }
    }

    // 2. Fallback: 用 Pexels 免费搜索（需要 API key，跳过）
    let _pexels_url = format!(
        "https://api.pexels.com/v1/search?query={}&per_page=1&size=small",
        urlencoding::encode(word)
    );
    // Pexels 需要 API key，跳过

    // 3. Fallback: 直接构造 Unsplash 图片 URL（基于关键词哈希）
    // 使用 Unsplash 的直接图片 URL 模式
    let direct_url = format!(
        "https://images.unsplash.com/photo-{}?w=800&h=500&fit=crop&auto=format",
        word_to_photo_id(word)
    );

    // 验证 URL 可访问
    if let Ok(r) = client.head(&direct_url).send().await {
        if r.status().is_success() {
            return Some(direct_url);
        }
    }

    // 4. 最终 fallback: Picsum（随机高质量图片）
    Some(format!("https://picsum.photos/seed/{}/800/500", word))
}

/// 将单词映射到 Unsplash photo ID（确定性，同一单词总是同一张图）
fn word_to_photo_id(word: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

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

    // 未匹配的单词用哈希生成一个确定性 ID
    let mut hasher = DefaultHasher::new();
    word.hash(&mut hasher);
    let hash = hasher.finish();

    // 使用一组高质量的通用图片 ID
    let generic_photos = [
        "1506905925346-21bda4d32df4",
        "1470071459604-3b5ec3a7fe05",
        "1464822759023-fed622ff2c3b",
        "1448375240586-882707db888b",
        "1477959858617-67f85cf4f1df",
        "1501139083538-0139583c060f",
        "1512820790803-83ca734da794",
        "1518837695005-2083093ee35b",
        "1507003211169-0a1dd7228f2d",
        "1419242902214-272b3f66ee7a",
    ];
    let idx = (hash as usize) % generic_photos.len();
    generic_photos[idx].to_string()
}

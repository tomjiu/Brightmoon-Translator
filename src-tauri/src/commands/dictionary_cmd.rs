// Multi-source Dictionary Commands - 多源词典聚合查询

use crate::models::dictionary::{Definition, DictionaryResult, Meaning};
use crate::services::multi_dictionary::MultiSourceDictionary;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;

/// 完整词典结果（包含所有源的数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComprehensiveEntry {
    pub word: String,
    pub phonetics: Vec<PhoneticInfo>,
    pub chinese_translation: Option<String>,
    pub english_definitions: Vec<String>,
    pub oxford_definition: Option<String>,
    pub online_meanings: Vec<OnlineMeaning>,
    pub gpt_analysis: Option<String>,
    pub audio_url: Option<String>,
    pub us_audio_url: Option<String>,
    pub uk_audio_url: Option<String>,
    pub examples: Vec<BilingualExample>,
    pub collins_entries: Vec<CollinsEntry>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BilingualExample {
    pub en: String,
    pub zh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneticInfo {
    pub text: Option<String>,
    pub audio: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMeaning {
    pub part_of_speech: String,
    pub definitions: Vec<OnlineDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineDefinition {
    pub definition: String,
    pub example: Option<String>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
}

/// 有道词典解析结果
struct YoudaoResult {
    chinese_translations: Vec<String>,
    examples: Vec<BilingualExample>,
    audio_url: Option<String>,
    phonetic: Option<String>,
    collins_entries: Vec<CollinsEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollinsEntry {
    pub pos: String,
    pub pos_cn: String,
    pub english_def: String,
    pub examples: Vec<BilingualExample>,
}

/// 多源聚合查询 — 并行查询所有源，合并最全结果
#[tauri::command]
pub async fn lookup_word_multi_source(
    word: String,
    state: State<'_, crate::AppState>,
) -> Result<ComprehensiveEntry, String> {
    let pool = state.ecdict_pool.as_ref();
    let vocab_db = state.event_store.as_ref().map(|s| s.pool());

    // 并行查询 ECDICT + DictionaryAPI.dev + 有道
    let (ecdict_result, online_result, youdao_result) = tokio::join!(
        async {
            match pool {
                Some(p) => lookup_ecdict(&word, p).await,
                None => Err("no pool".into()),
            }
        },
        async {
            let dict = MultiSourceDictionary::new();
            dict.lookup(&word).await.ok().and_then(|mut v| {
                if v.is_empty() {
                    None
                } else {
                    Some(v.remove(0))
                }
            })
        },
        async {
            let dict = MultiSourceDictionary::new();
            parse_youdao(&dict, &word).await
        }
    );

    // 串行查本地 Oxford + GPT4（毫秒级）
    let oxford_result = match vocab_db {
        Some(p) => lookup_oxford(&word, p).await.ok(),
        None => None,
    };
    let gpt_result = match vocab_db {
        Some(p) => lookup_gpt_dict(&word, p).await.ok(),
        None => None,
    };

    // 合并
    let mut entry = ComprehensiveEntry {
        word: word.clone(),
        phonetics: Vec::new(),
        chinese_translation: None,
        english_definitions: Vec::new(),
        oxford_definition: None,
        online_meanings: Vec::new(),
        gpt_analysis: None,
        audio_url: None,
        us_audio_url: None,
        uk_audio_url: None,
        examples: Vec::new(),
        collins_entries: Vec::new(),
        sources: Vec::new(),
    };

    // 1. ECDICT
    if let Ok((chinese, english, phonetic)) = &ecdict_result {
        entry.chinese_translation = chinese.clone();
        entry.english_definitions = english.clone();
        if let Some(p) = phonetic {
            let clean = p.trim().trim_start_matches('/').trim_end_matches('/');
            if !clean.is_empty() {
                entry.phonetics.push(PhoneticInfo {
                    text: Some(clean.to_string()),
                    audio: None,
                    source: "ECDICT".into(),
                });
            }
        }
        entry.sources.push("ECDICT".into());
    }

    // 有道：音频、音标、中文释义、例句、柯林斯
    if let Some(youdao) = youdao_result {
        if let Some(url) = &youdao.audio_url {
            entry.audio_url = Some(url.clone());
        }
        if let Some(phonetic) = &youdao.phonetic {
            entry.phonetics.push(PhoneticInfo {
                text: Some(phonetic.clone()),
                audio: youdao.audio_url.clone(),
                source: "有道".into(),
            });
        }
        if !youdao.chinese_translations.is_empty() {
            let youdao_zh = youdao.chinese_translations.join("；");
            if entry
                .chinese_translation
                .as_ref()
                .map_or(true, |existing| youdao_zh.len() > existing.len())
            {
                entry.chinese_translation = Some(youdao_zh);
            }
        }
        entry.examples = youdao.examples;
        entry.collins_entries = youdao.collins_entries;
        if !entry.collins_entries.is_empty() {
            entry.sources.push("柯林斯".into());
        }
        entry.sources.push("有道".into());
    }

    // 如果仍然没有音频，用有道 dictvoice 端点构造（对所有单词有效）
    if entry.audio_url.is_none() {
        entry.audio_url = Some(format!(
            "https://dict.youdao.com/dictvoice?audio={}&type=2",
            urlencoding::encode(&word)
        ));
    }
    // 美音/英音
    entry.us_audio_url = Some(format!(
        "https://dict.youdao.com/dictvoice?audio={}&type=2",
        urlencoding::encode(&word)
    ));
    entry.uk_audio_url = Some(format!(
        "https://dict.youdao.com/dictvoice?audio={}&type=1",
        urlencoding::encode(&word)
    ));

    // 3. Oxford
    if let Some(def) = oxford_result {
        entry.oxford_definition = Some(def);
        entry.sources.push("Oxford".into());
    }

    // 4. GPT4-Dict
    if let Some(analysis) = gpt_result {
        entry.gpt_analysis = Some(analysis);
        entry.sources.push("GPT4-Dict".into());
    }

    // 5. DictionaryAPI.dev
    if let Some(online) = online_result {
        for p in &online.phonetics {
            if p.audio.is_some() && entry.audio_url.is_none() {
                entry.audio_url = p.audio.clone();
            }
            if p.text.is_some() {
                entry.phonetics.push(PhoneticInfo {
                    text: p.text.clone(),
                    audio: p.audio.clone(),
                    source: "DictionaryAPI.dev".into(),
                });
            }
        }
        for m in &online.meanings {
            entry.online_meanings.push(OnlineMeaning {
                part_of_speech: m.part_of_speech.clone(),
                definitions: m
                    .definitions
                    .iter()
                    .map(|d| OnlineDefinition {
                        definition: d.definition.clone(),
                        example: d.example.clone(),
                        synonyms: d.synonyms.clone(),
                        antonyms: d.antonyms.clone(),
                    })
                    .collect(),
            });
        }
        entry.sources.push("DictionaryAPI.dev".into());
    }

    if entry.sources.is_empty() {
        return Err(if pool.is_some() {
            format!("单词 '{}' 未找到", word)
        } else {
            "本地词典未加载".into()
        });
    }

    Ok(entry)
}

/// 解析有道词典 JSON
async fn parse_youdao(dict: &MultiSourceDictionary, word: &str) -> Option<YoudaoResult> {
    let raw = match dict.lookup_youdao(word).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Youdao request failed for '{}': {}", word, e);
            return None;
        },
    };
    let json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Youdao JSON parse failed for '{}': {}", word, e);
            return None;
        },
    };

    let mut result = YoudaoResult {
        chinese_translations: Vec::new(),
        examples: Vec::new(),
        audio_url: None,
        phonetic: None,
        collins_entries: Vec::new(),
    };

    // 音标 + 音频
    if let Some(ec) = json.get("ec") {
        if let Some(word_list) = ec
            .get("word")
            .and_then(|w| w.as_array())
            .and_then(|a| a.first())
        {
            if let Some(us) = word_list.get("usphone").and_then(|v| v.as_str()) {
                result.phonetic = Some(us.to_string());
            } else if let Some(uk) = word_list.get("ukphone").and_then(|v| v.as_str()) {
                result.phonetic = Some(uk.to_string());
            }
            // 有道音频字段是 usspeech/ukspeech（无连字符），值是相对路径
            if let Some(speech) = word_list.get("usspeech").and_then(|v| v.as_str()) {
                result.audio_url = Some(format!(
                    "https://dict.youdao.com/dictvoice?audio={}",
                    speech
                ));
            } else if let Some(speech) = word_list.get("ukspeech").and_then(|v| v.as_str()) {
                result.audio_url = Some(format!(
                    "https://dict.youdao.com/dictvoice?audio={}",
                    speech
                ));
            }
        }
    }

    // 如果 ec 没有音频，尝试 collins_primary
    if result.audio_url.is_none() {
        if let Some(gc) = json
            .get("collins_primary")
            .and_then(|v| v.get("gramcat"))
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
        {
            if let Some(url) = gc.get("audiourl").and_then(|v| v.as_str()) {
                result.audio_url = Some(url.to_string());
            }
        }
    }

    // 中文释义 (ec -> word -> trs)
    if let Some(ec) = json.get("ec") {
        if let Some(word_list) = ec.get("word").and_then(|w| w.as_array()) {
            for w in word_list {
                if let Some(trs) = w.get("trs").and_then(|t| t.as_array()) {
                    for tr in trs {
                        if let Some(tr_text) = tr
                            .get("tr")
                            .and_then(|t| t.as_array())
                            .and_then(|a| a.first())
                            .and_then(|v| v.get("l"))
                            .and_then(|l| l.get("i"))
                            .and_then(|i| i.as_array())
                            .and_then(|a| a.first())
                            .and_then(|v| v.as_str())
                        {
                            result.chinese_translations.push(tr_text.to_string());
                        }
                    }
                }
            }
        }
    }

    // 例句 (blng_sents_part -> sentence-pair)
    if let Some(sents) = json
        .get("blng_sents_part")
        .and_then(|v| v.get("sentence-pair"))
        .and_then(|v| v.as_array())
    {
        for pair in sents.iter().take(5) {
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
                result.examples.push(BilingualExample { en, zh });
            }
        }
    }

    // 柯林斯词典（权威英英释义 + 双语例句）
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
                                    // 去掉 HTML 标签
                                    let clean = s
                                        .replace("<b>", "")
                                        .replace("</b>", "")
                                        .replace("<em>", "")
                                        .replace("</em>", "");
                                    clean.trim().to_string()
                                })
                                .unwrap_or_default();

                            let mut collins_examples = Vec::new();
                            if let Some(sents) = te
                                .get("exam_sents")
                                .and_then(|v| v.get("sent"))
                                .and_then(|v| v.as_array())
                            {
                                for sent in sents.iter().take(3) {
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
                                        collins_examples.push(BilingualExample { en, zh });
                                    }
                                }
                            }

                            if !en_def.is_empty() {
                                result.collins_entries.push(CollinsEntry {
                                    pos,
                                    pos_cn,
                                    english_def: en_def,
                                    examples: collins_examples,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // 如果 ec 没有中文释义，尝试 simple word
    if result.chinese_translations.is_empty() {
        if let Some(simple) = json
            .get("simple")
            .and_then(|v| v.get("word"))
            .and_then(|w| w.as_array())
        {
            for w in simple {
                if let Some(value) = w.get("value").and_then(|v| v.as_str()) {
                    result.chinese_translations.push(value.to_string());
                }
            }
        }
    }

    if result.chinese_translations.is_empty()
        && result.examples.is_empty()
        && result.audio_url.is_none()
        && result.collins_entries.is_empty()
    {
        tracing::warn!("Youdao returned nothing useful for '{}'", word);
        return None;
    }

    tracing::info!(
        "Youdao '{}': zh_trans={}, examples={}, audio={}",
        word,
        result.chinese_translations.len(),
        result.examples.len(),
        result.audio_url.is_some()
    );

    Some(result)
}

/// 查询 ECDICT
async fn lookup_ecdict(
    word: &str,
    pool: &sqlx::SqlitePool,
) -> Result<(Option<String>, Vec<String>, Option<String>), String> {
    let row =
        sqlx::query("SELECT word, phonetic, definition, translation FROM stardict WHERE word = ?1")
            .bind(word)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    if let Some(row) = row {
        let phonetic: Option<String> = row.get("phonetic");
        let definition: Option<String> = row.get("definition");
        let translation: Option<String> = row.get("translation");

        let english: Vec<String> = definition
            .as_deref()
            .map(|d| {
                d.split('\n')
                    .filter(|s| !s.trim().is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        Ok((translation, english, phonetic))
    } else {
        Err("not found".into())
    }
}

/// 查询 Oxford 词典
pub async fn lookup_oxford(word: &str, pool: &sqlx::SqlitePool) -> Result<String, String> {
    // Oxford 表的小写匹配
    let word_lower = word.to_lowercase();
    let row = sqlx::query("SELECT meaning FROM oxford_dict WHERE word = ?1 OR word = ?2")
        .bind(word)
        .bind(&word_lower)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

    row.map(|r| {
        let m: String = r.get("meaning");
        m.trim().to_string()
    })
    .ok_or_else(|| "not found".into())
}

/// 查询 GPT4-Dict
pub async fn lookup_gpt_dict(word: &str, pool: &sqlx::SqlitePool) -> Result<String, String> {
    let word_lower = word.to_lowercase();
    let row = sqlx::query("SELECT content FROM gpt_dict WHERE word = ?1 OR word = ?2")
        .bind(word)
        .bind(&word_lower)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

    row.map(|r| {
        let c: String = r.get("content");
        c.trim().to_string()
    })
    .ok_or_else(|| "not found".into())
}

/// 搜索建议（带中文释义预览）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionItem {
    pub word: String,
    pub preview: Option<String>,
}

/// 搜索建议（ECDICT 前缀匹配 + 释义预览）
#[tauri::command]
pub async fn search_word_suggestions(
    query: String,
    limit: i32,
    state: State<'_, crate::AppState>,
) -> Result<Vec<SuggestionItem>, String> {
    let pool = state.ecdict_pool.as_ref().ok_or("本地词典数据库未加载")?;

    let pattern = format!("{}%", query);

    let rows = sqlx::query("SELECT word, translation FROM stardict WHERE word LIKE ?1 ORDER BY frq DESC, word ASC LIMIT ?2")
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("查询建议失败: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|r| SuggestionItem {
            word: r.get("word"),
            preview: r.get("translation"),
        })
        .collect())
}

/// 检查词典数据是否已导入
#[tauri::command]
pub async fn check_dictionary_imported(
    state: State<'_, crate::AppState>,
) -> Result<DictionaryImportStatus, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();

    let oxford_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oxford_dict")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let gpt_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gpt_dict")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let vocab_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM core_vocabulary")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    Ok(DictionaryImportStatus {
        oxford_count,
        gpt_count,
        vocab_count,
        imported: oxford_count > 0 || vocab_count > 0,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryImportStatus {
    pub oxford_count: i64,
    pub gpt_count: i64,
    pub vocab_count: i64,
    pub imported: bool,
}

/// 导入 Oxford、GPT4-Dict、核心词库（一次性操作，数据持久化）
#[tauri::command]
pub async fn import_dictionary_data(state: State<'_, crate::AppState>) -> Result<String, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let ecdict_pool = state.ecdict_pool.as_ref().ok_or("ECDICT 未连接")?;

    // 创建表（IF NOT EXISTS 保证幂等）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS oxford_dict (word TEXT PRIMARY KEY, meaning TEXT NOT NULL)",
    )
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS gpt_dict (word TEXT PRIMARY KEY, content TEXT NOT NULL)",
    )
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query("CREATE TABLE IF NOT EXISTS core_vocabulary (word TEXT PRIMARY KEY, frequency_rank INTEGER NOT NULL, frq INTEGER, bnc INTEGER, collins INTEGER, oxford INTEGER, tag TEXT)")
        .execute(pool).await.map_err(|e| e.to_string())?;

    // 跳过已导入的
    let existing_oxford: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oxford_dict")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let existing_vocab: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM core_vocabulary")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let mut oxford_count = 0i64;
    let mut gpt_count = 0i64;
    let mut vocab_count = 0i64;

    // 导入 Oxford（跳过已有数据）
    if existing_oxford == 0 {
        let oxford_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("dictionaries")
            .join("oxford-41k")
            .join("oedict.sql");
        if oxford_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&oxford_path) {
                let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
                for line in content.lines() {
                    if !line.starts_with('(') {
                        continue;
                    }
                    if let Some((word, meaning)) = parse_oxford_line(line) {
                        let w = word.trim().trim_start_matches('\n').to_lowercase();
                        let m = meaning.trim();
                        if !w.is_empty() && !m.is_empty() {
                            sqlx::query(
                                "INSERT OR IGNORE INTO oxford_dict (word, meaning) VALUES (?, ?)",
                            )
                            .bind(&w)
                            .bind(m)
                            .execute(&mut *tx)
                            .await
                            .ok();
                            oxford_count += 1;
                        }
                    }
                    if oxford_count % 5000 == 0 && oxford_count > 0 {
                        tx.commit().await.map_err(|e| e.to_string())?;
                        tx = pool.begin().await.map_err(|e| e.to_string())?;
                    }
                }
                tx.commit().await.map_err(|e| e.to_string())?;
            }
        }
    }

    // 导入 GPT4-Dict
    let existing_gpt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gpt_dict")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    if existing_gpt == 0 {
        let gpt_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("dictionaries")
            .join("gpt4-dict")
            .join("gptwords.json");
        if gpt_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&gpt_path) {
                let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
                for line in content.lines() {
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(entry) = serde_json::from_str::<GptEntry>(line) {
                        sqlx::query("INSERT OR IGNORE INTO gpt_dict (word, content) VALUES (?, ?)")
                            .bind(entry.word.to_lowercase())
                            .bind(&entry.content)
                            .execute(&mut *tx)
                            .await
                            .ok();
                        gpt_count += 1;
                    }
                    if gpt_count % 1000 == 0 && gpt_count > 0 {
                        tx.commit().await.map_err(|e| e.to_string())?;
                        tx = pool.begin().await.map_err(|e| e.to_string())?;
                    }
                }
                tx.commit().await.map_err(|e| e.to_string())?;
            }
        }
    }

    // 导入核心词库
    if existing_vocab == 0 {
        let rows = sqlx::query(
            "SELECT word, frq, bnc, collins, oxford, tag FROM stardict WHERE frq IS NOT NULL ORDER BY frq DESC LIMIT 15000",
        ).fetch_all(ecdict_pool).await.map_err(|e| e.to_string())?;

        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        for (rank, row) in rows.iter().enumerate() {
            let word: String = row.get("word");
            let frq: Option<i64> = row.get("frq");
            let bnc: Option<i64> = row.get("bnc");
            let collins: Option<i64> = row.get("collins");
            let oxford: Option<i64> = row.get("oxford");
            let tag: Option<String> = row.get("tag");

            sqlx::query("INSERT OR IGNORE INTO core_vocabulary (word, frequency_rank, frq, bnc, collins, oxford, tag) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(&word).bind((rank + 1) as i64).bind(frq).bind(bnc).bind(collins).bind(oxford).bind(&tag)
                .execute(&mut *tx).await.ok();
            vocab_count += 1;

            if vocab_count % 5000 == 0 {
                tx.commit().await.map_err(|e| e.to_string())?;
                tx = pool.begin().await.map_err(|e| e.to_string())?;
            }
        }
        tx.commit().await.map_err(|e| e.to_string())?;
    }

    Ok(format!(
        "导入完成: Oxford {}, GPT4 {}, 核心词库 {}",
        oxford_count, gpt_count, vocab_count
    ))
}

#[derive(Deserialize)]
struct GptEntry {
    word: String,
    content: String,
}

/// 解析 Oxford SQL INSERT 行
/// 格式: (id, 'letter', 'word', 'meaning')
fn parse_oxford_line(line: &str) -> Option<(String, String)> {
    // 去掉开头的 ( 和结尾的 ),
    let inner = line.trim().trim_start_matches('(').trim_end_matches(',');
    let inner = inner.trim_end_matches(')');

    // 找到第3个和第4个 ' 分隔的字段
    let parts: Vec<&str> = inner.splitn(4, "', '").collect();
    if parts.len() >= 4 {
        let word = parts[2].trim_start_matches('\'').trim_end_matches('\'');
        let meaning = parts[3].trim_start_matches('\'').trim_end_matches('\'');
        Some((word.to_string(), meaning.to_string()))
    } else {
        // 备用解析：按逗号分割（前两个是数字和单字母）
        let in_quote = false;
        let mut fields = Vec::new();
        let mut current = String::new();
        let mut quote_count = 0;

        for ch in inner.chars() {
            match ch {
                '\'' => {
                    quote_count += 1;
                    if quote_count % 2 == 0 {
                        // 结束引号
                        fields.push(current.clone());
                        current.clear();
                    }
                },
                ',' if !in_quote && quote_count % 2 == 0 => {
                    // 字段分隔
                },
                _ if quote_count % 2 == 1 => {
                    current.push(ch);
                },
                _ => {},
            }
        }

        if fields.len() >= 4 {
            Some((fields[2].clone(), fields[3].clone()))
        } else {
            None
        }
    }
}

/// 搜索建议（旧接口兼容）
#[tauri::command]
pub async fn lookup_word_detail(
    word: String,
    state: State<'_, crate::AppState>,
) -> Result<DictionaryResult, String> {
    let pool = state.ecdict_pool.as_ref().ok_or("本地词典数据库未加载")?;

    let row =
        sqlx::query("SELECT word, phonetic, definition, translation FROM stardict WHERE word = ?1")
            .bind(&word)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    if let Some(row) = row {
        let word: String = row.get("word");
        let phonetic: Option<String> = row.get("phonetic");
        let definition: Option<String> = row.get("definition");
        let translation: Option<String> = row.get("translation");

        let mut meanings = Vec::new();

        if let Some(definition) = &definition {
            let defs: Vec<&str> = definition.split('\n').collect();
            let definitions: Vec<Definition> = defs
                .into_iter()
                .filter(|d| !d.trim().is_empty())
                .map(|d| Definition {
                    definition: d.to_string(),
                    example: None,
                    synonyms: vec![],
                    antonyms: vec![],
                })
                .collect();

            if !definitions.is_empty() {
                meanings.push(Meaning {
                    part_of_speech: "英文释义".to_string(),
                    definitions,
                });
            }
        }

        if let Some(translation) = &translation {
            meanings.push(Meaning {
                part_of_speech: "中文释义".to_string(),
                definitions: vec![Definition {
                    definition: translation.clone(),
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

/// 模糊搜索
#[tauri::command]
pub async fn fuzzy_search_words(
    query: String,
    limit: i32,
    state: State<'_, crate::AppState>,
) -> Result<Vec<String>, String> {
    let pool = state.ecdict_pool.as_ref().ok_or("本地词典数据库未加载")?;

    let pattern = format!("%{}%", query);
    let prefix_pattern = format!("{}%", query);

    let rows = sqlx::query(
        r#"
        SELECT word
        FROM stardict
        WHERE word LIKE ?1
           OR translation LIKE ?1
        ORDER BY
            CASE
                WHEN word = ?2 THEN 0
                WHEN word LIKE ?3 THEN 1
                ELSE 2
            END,
            word ASC
        LIMIT ?4
        "#,
    )
    .bind(&pattern)
    .bind(&query)
    .bind(&prefix_pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(|r| r.get("word")).collect())
}

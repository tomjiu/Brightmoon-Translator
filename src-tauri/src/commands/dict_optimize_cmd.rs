// Dictionary Optimization Commands - 词典数据优化（压缩/分片）

use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;

/// 词典统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictStats {
    pub total_words: i64,
    pub high_freq_words: i64, // frq <= 5000
    pub mid_freq_words: i64,  // 5000 < frq <= 15000
    pub low_freq_words: i64,  // frq > 15000
    pub no_freq_words: i64,   // frq IS NULL
    pub total_size_mb: f64,
}

/// 分片信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShardInfo {
    pub letter: String,
    pub word_count: i32,
    pub file_name: String,
}

/// 清单文件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    pub created_at: String,
    pub total_words: i32,
    pub shards: Vec<ShardInfo>,
}

/// 获取词典统计信息
#[tauri::command]
pub async fn get_dict_stats(state: State<'_, crate::AppState>) -> Result<DictStats, String> {
    let ecdict_pool = state.ecdict_pool.as_ref().ok_or("ECDICT 未连接")?;

    let total_words: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stardict")
        .fetch_one(ecdict_pool)
        .await
        .unwrap_or(0);

    let high_freq: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM stardict WHERE frq IS NOT NULL AND frq <= 5000")
            .fetch_one(ecdict_pool)
            .await
            .unwrap_or(0);

    let mid_freq: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM stardict WHERE frq IS NOT NULL AND frq > 5000 AND frq <= 15000",
    )
    .fetch_one(ecdict_pool)
    .await
    .unwrap_or(0);

    let low_freq: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM stardict WHERE frq IS NOT NULL AND frq > 15000")
            .fetch_one(ecdict_pool)
            .await
            .unwrap_or(0);

    let no_freq: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stardict WHERE frq IS NULL")
        .fetch_one(ecdict_pool)
        .await
        .unwrap_or(0);

    // 获取数据库文件大小（估算）
    let page_count: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(ecdict_pool)
        .await
        .unwrap_or(0);
    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(ecdict_pool)
        .await
        .unwrap_or(4096);
    let total_size_mb = (page_count * page_size) as f64 / 1024.0 / 1024.0;

    Ok(DictStats {
        total_words,
        high_freq_words: high_freq,
        mid_freq_words: mid_freq,
        low_freq_words: low_freq,
        no_freq_words: no_freq,
        total_size_mb,
    })
}

/// 导出压缩版词典（移除低频词，精简字段）
#[tauri::command]
pub async fn export_compressed_dict(
    state: State<'_, crate::AppState>,
    output_path: String,
    max_rank: i32,
) -> Result<ExportResult, String> {
    let ecdict_pool = state.ecdict_pool.as_ref().ok_or("ECDICT 未连接")?;

    // 创建压缩后的 SQLite 数据库
    let output_pool = sqlx::SqlitePool::connect(&format!("sqlite:{output_path}?mode=rwc"))
        .await
        .map_err(|e| format!("创建输出数据库失败: {e}"))?;

    // 创建表结构
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS stardict (
            word TEXT PRIMARY KEY,
            phonetic TEXT,
            definition TEXT,
            translation TEXT,
            pos TEXT,
            frq INTEGER
        )",
    )
    .execute(&output_pool)
    .await
    .map_err(|e| e.to_string())?;

    // 复制高频词
    let rows = sqlx::query(
        "SELECT word, phonetic, definition, translation, pos, frq
         FROM stardict
         WHERE frq IS NOT NULL AND frq <= ?
         ORDER BY frq ASC",
    )
    .bind(max_rank)
    .fetch_all(ecdict_pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut count = 0;
    for row in &rows {
        let word: String = row.get("word");
        let phonetic: Option<String> = row.get("phonetic");
        let definition: Option<String> = row.get("definition");
        let translation: Option<String> = row.get("translation");
        let pos: Option<String> = row.get("pos");
        let frq: Option<i32> = row.get("frq");

        // 精简释义（截断过长内容）
        let short_definition = definition.map(|d| {
            if d.len() > 500 {
                d[..500].to_string()
            } else {
                d
            }
        });

        let short_translation = translation.map(|t| {
            if t.len() > 300 {
                t[..300].to_string()
            } else {
                t
            }
        });

        sqlx::query(
            "INSERT OR IGNORE INTO stardict (word, phonetic, definition, translation, pos, frq)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&word)
        .bind(&phonetic)
        .bind(&short_definition)
        .bind(&short_translation)
        .bind(&pos)
        .bind(frq)
        .execute(&output_pool)
        .await
        .ok();

        count += 1;
    }

    // 创建索引
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_frq ON stardict(frq)")
        .execute(&output_pool)
        .await
        .ok();

    // 压缩数据库
    sqlx::query("VACUUM").execute(&output_pool).await.ok();

    output_pool.close().await;

    Ok(ExportResult {
        exported_words: count,
        output_path,
    })
}

/// 导出结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub exported_words: i32,
    pub output_path: String,
}

/// 按字母分片导出词典
#[tauri::command]
pub async fn export_dict_shards(
    state: State<'_, crate::AppState>,
    output_dir: String,
) -> Result<Manifest, String> {
    let ecdict_pool = state.ecdict_pool.as_ref().ok_or("ECDICT 未连接")?;

    // 创建输出目录
    std::fs::create_dir_all(&output_dir).map_err(|e| format!("创建目录失败: {e}"))?;

    let mut shards = Vec::new();
    let mut total_words = 0;

    // 按首字母分组
    let letters = "abcdefghijklmnopqrstuvwxyz";

    for letter in letters.chars() {
        let pattern = format!("{letter}%");
        let count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM stardict WHERE word LIKE ?")
            .bind(&pattern)
            .fetch_one(ecdict_pool)
            .await
            .unwrap_or(0);

        if count == 0 {
            continue;
        }

        // 创建分片数据库
        let shard_name = format!("ecdict_{letter}.db");
        let shard_path = format!("{output_dir}/{shard_name}");
        let shard_pool = sqlx::SqlitePool::connect(&format!("sqlite:{shard_path}?mode=rwc"))
            .await
            .map_err(|e| format!("创建分片数据库失败: {e}"))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS stardict (
                word TEXT PRIMARY KEY,
                phonetic TEXT,
                definition TEXT,
                translation TEXT,
                pos TEXT,
                frq INTEGER
            )",
        )
        .execute(&shard_pool)
        .await
        .map_err(|e| e.to_string())?;

        // 复制该字母的单词
        let rows = sqlx::query(
            "SELECT word, phonetic, definition, translation, pos, frq
             FROM stardict WHERE word LIKE ? ORDER BY word",
        )
        .bind(&pattern)
        .fetch_all(ecdict_pool)
        .await
        .map_err(|e| e.to_string())?;

        for row in &rows {
            sqlx::query(
                "INSERT OR IGNORE INTO stardict (word, phonetic, definition, translation, pos, frq)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(row.get::<String, _>("word"))
            .bind(row.get::<Option<String>, _>("phonetic"))
            .bind(row.get::<Option<String>, _>("definition"))
            .bind(row.get::<Option<String>, _>("translation"))
            .bind(row.get::<Option<String>, _>("pos"))
            .bind(row.get::<Option<i32>, _>("frq"))
            .execute(&shard_pool)
            .await
            .ok();
        }

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_frq ON stardict(frq)")
            .execute(&shard_pool)
            .await
            .ok();

        sqlx::query("VACUUM").execute(&shard_pool).await.ok();

        shard_pool.close().await;

        shards.push(ShardInfo {
            letter: letter.to_string(),
            word_count: count,
            file_name: shard_name,
        });

        total_words += count;
    }

    // 生成清单文件
    let manifest = Manifest {
        version: "1.0".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        total_words,
        shards,
    };

    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|e| format!("序列化清单失败: {e}"))?;

    let manifest_path = format!("{output_dir}/manifest.json");
    std::fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("写入清单文件失败: {e}"))?;

    Ok(manifest)
}

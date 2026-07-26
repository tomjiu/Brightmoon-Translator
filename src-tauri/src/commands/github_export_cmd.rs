// GitHub Data Export Commands - 导出数据到 GitHub 格式

use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;

/// 导出结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubExportResult {
    pub total_words: i32,
    pub shards_created: i32,
    pub output_dir: String,
}

/// GitHub 分片信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubShardInfo {
    pub letter: String,
    pub word_count: i32,
    pub json_file: String,
    pub gz_file: String,
}

/// 清单文件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubManifest {
    pub version: String,
    pub created_at: String,
    pub total_words: i32,
    pub shards: Vec<GitHubShardInfo>,
    pub download_base_url: String,
}

/// 导出词典到 GitHub 格式（JSON + GZ 压缩）
#[tauri::command]
pub async fn export_for_github(
    state: State<'_, crate::AppState>,
    output_dir: String,
    max_rank: Option<i32>,
) -> Result<GitHubExportResult, String> {
    let ecdict_pool = state.ecdict_pool.as_ref().ok_or("ECDICT 未连接")?;
    let max_rank = max_rank.unwrap_or(50000);

    // 创建输出目录
    let ecdict_dir = format!("{}/ecdict", output_dir);
    std::fs::create_dir_all(&ecdict_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let mut shards = Vec::new();
    let mut total_words = 0;

    // 按首字母分组导出
    let letters = "abcdefghijklmnopqrstuvwxyz";

    for letter in letters.chars() {
        let pattern = format!("{}%", letter);

        let rows = sqlx::query(
            "SELECT word, phonetic, definition, translation, pos, frq
             FROM stardict
             WHERE word LIKE ? AND (frq IS NULL OR frq <= ?)
             ORDER BY word",
        )
        .bind(&pattern)
        .bind(max_rank)
        .fetch_all(ecdict_pool)
        .await
        .map_err(|e| e.to_string())?;

        if rows.is_empty() {
            continue;
        }

        // 构造 JSON 数组
        let entries: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "word": row.get::<String, _>("word"),
                    "phonetic": row.get::<Option<String>, _>("phonetic"),
                    "definition": row.get::<Option<String>, _>("definition"),
                    "translation": row.get::<Option<String>, _>("translation"),
                    "pos": row.get::<Option<String>, _>("pos"),
                    "frq": row.get::<Option<i32>, _>("frq"),
                })
            })
            .collect();

        let count = entries.len() as i32;
        total_words += count;

        // 写入 JSON 文件
        let json_file = format!("ecdict_{}.json", letter);
        let json_path = format!("{}/{}", ecdict_dir, json_file);
        let json_content =
            serde_json::to_string(&entries).map_err(|e| format!("序列化失败: {}", e))?;
        std::fs::write(&json_path, &json_content).map_err(|e| format!("写入文件失败: {}", e))?;

        // 压缩为 GZ
        let gz_file = format!("ecdict_{}.json.gz", letter);
        let gz_path = format!("{}/{}", ecdict_dir, gz_file);
        compress_gz(&json_content, &gz_path)?;

        shards.push(GitHubShardInfo {
            letter: letter.to_string(),
            word_count: count,
            json_file,
            gz_file,
        });
    }

    // 生成清单文件
    let manifest = GitHubManifest {
        version: "1.0".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        total_words,
        shards,
        download_base_url: format!(
            "https://raw.githubusercontent.com/YOUR_USERNAME/moontranslator-data/main/ecdict"
        ),
    };

    let manifest_path = format!("{}/manifest.json", ecdict_dir);
    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|e| format!("序列化清单失败: {}", e))?;
    std::fs::write(&manifest_path, manifest_json).map_err(|e| format!("写入清单失败: {}", e))?;

    Ok(GitHubExportResult {
        total_words,
        shards_created: 26,
        output_dir: ecdict_dir,
    })
}

/// 导出 AI 内容缓存到 GitHub 格式
#[tauri::command]
pub async fn export_ai_cache_for_github(
    state: State<'_, crate::AppState>,
    output_dir: String,
    limit: Option<i32>,
) -> Result<i32, String> {
    let store = state.event_store.as_ref().ok_or("数据库未初始化")?;
    let pool = store.pool();
    let limit = limit.unwrap_or(1000);

    let ai_cache_dir = format!("{}/ai-cache", output_dir);
    std::fs::create_dir_all(&ai_cache_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    // 获取有 AI 内容的卡牌
    let rows = sqlx::query(
        "SELECT word, ai_content FROM cards
         WHERE ai_content IS NOT NULL AND ai_content != ''
         ORDER BY RANDOM() LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut count = 0;
    let mut cache_entries = Vec::new();

    for row in &rows {
        let word: String = row.get("word");
        let ai_content_str: Option<String> = row.get("ai_content");

        if let Some(content) = ai_content_str {
            if let Ok(ai) = serde_json::from_str::<serde_json::Value>(&content) {
                cache_entries.push(serde_json::json!({
                    "word": word,
                    "content": ai,
                }));
                count += 1;
            }
        }
    }

    // 写入缓存文件
    let cache_path = format!("{}/common_{}.json", ai_cache_dir, limit);
    let cache_json =
        serde_json::to_string_pretty(&cache_entries).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(&cache_path, cache_json).map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(count)
}

/// GZ 压缩
fn compress_gz(data: &str, output_path: &str) -> Result<(), String> {
    use std::io::Write;
    let file = std::fs::File::create(output_path).map_err(|e| format!("创建文件失败: {}", e))?;
    let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    encoder
        .write_all(data.as_bytes())
        .map_err(|e| format!("压缩失败: {}", e))?;
    encoder
        .finish()
        .map_err(|e| format!("完成压缩失败: {}", e))?;
    Ok(())
}

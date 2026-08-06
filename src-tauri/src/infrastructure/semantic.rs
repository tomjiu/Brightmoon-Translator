// Semantic Similarity - T9 轻量语义向量
//
// 基于字符 n-gram 哈希向量的语义相似度,无需外部 embedding API。
// 词向量从 ECDICT 的 translation/definition 文本构建,余弦相似度选干扰项。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

/// 语义向量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticVector {
    /// 维度
    pub dim: usize,
    /// 稀疏向量（feature_id -> 权重）
    pub weights: HashMap<u32, f32>,
}

/// 从文本构建字符 n-gram 哈希向量（2-gram + 3-gram，TF 权重）
pub fn build_vector(text: &str, dim: usize) -> SemanticVector {
    let mut weights: HashMap<u32, f32> = HashMap::new();
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();

    if chars.len() < 2 {
        return SemanticVector { dim, weights };
    }

    // 2-gram
    for pair in chars.windows(2) {
        let mut key = [0u8; 4];
        let s: String = pair.iter().collect();
        let mut i = 0;
        for b in s.bytes() {
            if i < 4 {
                key[i] = b;
                i += 1;
            }
        }
        let h = fnv1a(&key);
        *weights.entry(h % dim as u32).or_insert(0.0) += 1.0;
    }

    // 3-gram
    if chars.len() >= 3 {
        for triple in chars.windows(3) {
            let s: String = triple.iter().collect();
            let mut key = [0u8; 4];
            let mut i = 0;
            for b in s.bytes() {
                if i < 4 {
                    key[i] = b;
                    i += 1;
                }
            }
            let h = fnv1a(&key);
            *weights.entry(h % dim as u32).or_insert(0.0) += 0.5;
        }
    }

    // TF 归一化
    let norm: f32 = weights.values().map(|w| w * w).sum::<f32>().sqrt();
    if norm > 0.0 {
        for w in weights.values_mut() {
            *w /= norm;
        }
    }

    SemanticVector { dim, weights }
}

/// FNV-1a 哈希
fn fnv1a(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &b in bytes {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

impl SemanticVector {
    /// 余弦相似度
    pub fn cosine(&self, other: &SemanticVector) -> f32 {
        if self.weights.is_empty() || other.weights.is_empty() {
            return 0.0;
        }
        let (small, large) = if self.weights.len() < other.weights.len() {
            (&self.weights, &other.weights)
        } else {
            (&other.weights, &self.weights)
        };
        let mut dot = 0.0f32;
        for (k, v) in small.iter() {
            if let Some(v2) = large.get(k) {
                dot += v * v2;
            }
        }
        // 已做 TF 归一化,无需再除范数
        dot.clamp(0.0, 1.0)
    }

    /// 序列化为 JSON 字符串
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// 从 JSON 反序列化
    pub fn from_json(s: &str) -> Result<Self> {
        serde_json::from_str(s).context("反序列化语义向量失败")
    }
}

/// 存入 embeddings 表
pub async fn upsert_embedding(
    pool: &SqlitePool,
    word: &str,
    source: &str,
    vector: &SemanticVector,
) -> Result<()> {
    let json = vector.to_json()?;
    sqlx::query(
        r#"
        INSERT INTO embeddings (word, source, vector, dim, created_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(word, source) DO UPDATE SET
            vector = excluded.vector,
            dim = excluded.dim
        "#,
    )
    .bind(word)
    .bind(source)
    .bind(&json)
    .bind(vector.dim as i64)
    .bind(chrono::Utc::now().timestamp())
    .execute(pool)
    .await?;
    Ok(())
}

/// 读取单个词的向量
pub async fn load_embedding(
    pool: &SqlitePool,
    word: &str,
    source: &str,
) -> Result<Option<SemanticVector>> {
    let row = sqlx::query(
        "SELECT vector FROM embeddings WHERE word = ? AND source = ?",
    )
    .bind(word)
    .bind(source)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        let json: String = row.try_get("vector")?;
        Ok(Some(SemanticVector::from_json(&json)?))
    } else {
        Ok(None)
    }
}

/// 批量读取多个词的向量
pub async fn load_embeddings(
    pool: &SqlitePool,
    words: &[String],
    source: &str,
) -> Result<HashMap<String, SemanticVector>> {
    if words.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = vec!["?"; words.len()].join(",");
    let sql = format!(
        "SELECT word, vector FROM embeddings WHERE word IN ({}) AND source = ?",
        placeholders
    );
    let mut q = sqlx::query(&sql);
    for w in words {
        q = q.bind(w);
    }
    q = q.bind(source);

    let rows = q.fetch_all(pool).await?;
    let mut map = HashMap::new();
    for row in rows {
        let word: String = row.try_get("word")?;
        let json: String = row.try_get("vector")?;
        if let Ok(v) = SemanticVector::from_json(&json) {
            map.insert(word, v);
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = build_vector("苹果 一种水果 甜", 128);
        let b = build_vector("苹果 水果", 128);
        let c = build_vector("汽车 发动机 速度", 128);

        let sim_ab = a.cosine(&b);
        let sim_ac = a.cosine(&c);
        assert!(sim_ab > sim_ac, "语义相近的词相似度应更高: {sim_ab} vs {sim_ac}");
        assert!((0.0..=1.0).contains(&sim_ab));
    }

    #[test]
    fn test_vector_roundtrip() {
        let v = build_vector("hello world hello", 64);
        let json = v.to_json().unwrap();
        let v2 = SemanticVector::from_json(&json).unwrap();
        assert_eq!(v.dim, v2.dim);
        assert!(v.cosine(&v2) > 0.999);
    }

    #[tokio::test]
    async fn test_upsert_load() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
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
        .execute(&pool)
        .await
        .unwrap();

        let v = build_vector("test 测试", 32);
        upsert_embedding(&pool, "test", "ecdict", &v).await.unwrap();

        let loaded = load_embedding(&pool, "test", "ecdict").await.unwrap().unwrap();
        assert!(v.cosine(&loaded) > 0.999);

        // 再次 upsert 覆盖
        let v2 = build_vector("test 新的", 32);
        upsert_embedding(&pool, "test", "ecdict", &v2).await.unwrap();
        let loaded2 = load_embedding(&pool, "test", "ecdict").await.unwrap().unwrap();
        assert!(v2.cosine(&loaded2) > 0.999);
    }
}

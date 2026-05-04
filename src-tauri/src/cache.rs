use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTranslation {
    pub results: Vec<(String, String)>, // (engine, text)
    pub timestamp: i64,
    pub hits: i64,
}

pub struct TranslationCache {
    conn: Arc<Mutex<Connection>>,
    max_size: usize,
    ttl_hours: i64,
}

fn cache_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    std::fs::create_dir_all(&path).ok();
    path.push("cache.db");
    path
}

impl TranslationCache {
    pub fn new(max_size: usize) -> Self {
        let conn = Connection::open(cache_path()).expect("Failed to open cache database");

        // Create table if not exists
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS translations (
                cache_key TEXT PRIMARY KEY,
                from_lang TEXT NOT NULL,
                to_lang TEXT NOT NULL,
                source_text TEXT NOT NULL,
                engine TEXT NOT NULL,
                translated_text TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                hits INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_timestamp ON translations(timestamp);
            CREATE INDEX IF NOT EXISTS idx_from_to ON translations(from_lang, to_lang);
            "
        ).expect("Failed to create cache table");

        Self {
            conn: Arc::new(Mutex::new(conn)),
            max_size,
            ttl_hours: 72, // 3 days default TTL
        }
    }

    fn make_key(text: &str, from: &str, to: &str) -> String {
        format!("{}|{}|{}", from, to, text)
    }

    pub async fn get(&self, text: &str, from: &str, to: &str) -> Option<CachedTranslation> {
        let key = Self::make_key(text, from, to);
        let conn = self.conn.lock().await;

        // Delete expired entries
        let cutoff = Utc::now().timestamp_millis() - (self.ttl_hours * 3600 * 1000);
        let _ = conn.execute(
            "DELETE FROM translations WHERE timestamp < ?1",
            params![cutoff],
        );

        // Query for cached results
        let mut stmt = conn
            .prepare(
                "SELECT engine, translated_text, timestamp, hits
                 FROM translations
                 WHERE cache_key = ?1
                 ORDER BY engine",
            )
            .ok()?;

        let results: Vec<(String, String)> = stmt
            .query_map(params![key], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        if results.is_empty() {
            return None;
        }

        // Get timestamp and hits from first row
        let mut stmt = conn
            .prepare(
                "SELECT timestamp, hits FROM translations WHERE cache_key = ?1 LIMIT 1",
            )
            .ok()?;

        let (timestamp, hits): (i64, i64) = stmt
            .query_row(params![key], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok()?;

        // Increment hit count
        let _ = conn.execute(
            "UPDATE translations SET hits = hits + 1 WHERE cache_key = ?1",
            params![key],
        );

        Some(CachedTranslation {
            results,
            timestamp,
            hits: hits + 1,
        })
    }

    pub async fn set(&self, text: &str, from: &str, to: &str, results: Vec<(String, String)>) {
        let key = Self::make_key(text, from, to);
        let conn = self.conn.lock().await;
        let timestamp = Utc::now().timestamp_millis();

        // Delete existing entries for this key
        let _ = conn.execute(
            "DELETE FROM translations WHERE cache_key = ?1",
            params![key],
        );

        // Insert new results
        for (engine, translated) in &results {
            let _ = conn.execute(
                "INSERT INTO translations (cache_key, from_lang, to_lang, source_text, engine, translated_text, timestamp, hits)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                params![key, from, to, text, engine, translated, timestamp],
            );
        }

        // Evict oldest entries if cache exceeds max size
        let count: i64 = conn
            .query_row("SELECT COUNT(DISTINCT cache_key) FROM translations", [], |row| row.get(0))
            .unwrap_or(0);

        if count > self.max_size as i64 {
            let to_delete = count - self.max_size as i64;
            let _ = conn.execute(
                "DELETE FROM translations WHERE cache_key IN (
                    SELECT cache_key FROM translations
                    GROUP BY cache_key
                    ORDER BY MIN(timestamp) ASC
                    LIMIT ?1
                )",
                params![to_delete],
            );
        }
    }

    pub async fn clear(&self) {
        let conn = self.conn.lock().await;
        let _ = conn.execute("DELETE FROM translations", []);
    }

    pub async fn size(&self) -> usize {
        let conn = self.conn.lock().await;
        conn.query_row(
            "SELECT COUNT(DISTINCT cache_key) FROM translations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0) as usize
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let conn = self.conn.lock().await;

        let total_entries: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT cache_key) FROM translations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let total_hits: i64 = conn
            .query_row("SELECT SUM(hits) FROM translations", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        // Get per-engine stats
        let mut stmt = conn
            .prepare("SELECT engine, COUNT(*), SUM(hits) FROM translations GROUP BY engine")
            .unwrap();

        let engine_stats: Vec<EngineStats> = stmt
            .query_map([], |row| {
                Ok(EngineStats {
                    engine: row.get(0)?,
                    entries: row.get(1)?,
                    hits: row.get(2)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        CacheStats {
            total_entries,
            total_hits,
            engine_stats,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub total_entries: i64,
    pub total_hits: i64,
    pub engine_stats: Vec<EngineStats>,
}

#[derive(Debug, Serialize)]
pub struct EngineStats {
    pub engine: String,
    pub entries: i64,
    pub hits: i64,
}

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Minimum interval (in milliseconds) between expired-entry cleanup sweeps.
/// Cleanup is skipped if the last sweep was less than this many ms ago.
const CLEANUP_INTERVAL_MS: i64 = 60_000; // 1 minute

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
    /// Timestamp (millis) of the last expired-entry cleanup sweep.
    last_cleanup: Arc<Mutex<i64>>,
}

fn cache_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!("Failed to create cache directory {:?}: {}", path, e);
    }
    path.push("cache.db");
    path
}

impl TranslationCache {
    pub fn new(max_size: usize) -> Self {
        let conn = match Connection::open(cache_path()) {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("Failed to open cache database: {}", e);
                // Create in-memory database as fallback
                Connection::open_in_memory().expect("Failed to create in-memory cache")
            }
        };

        // Create table if not exists
        if let Err(e) = conn.execute_batch(
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
            ",
        ) {
            tracing::error!("Failed to create cache table: {}", e);
        }

        Self {
            conn: Arc::new(Mutex::new(conn)),
            max_size,
            ttl_hours: 72, // 3 days default TTL
            last_cleanup: Arc::new(Mutex::new(0)),
        }
    }

    fn make_key(text: &str, from: &str, to: &str) -> String {
        format!("{}|{}|{}", from, to, text)
    }

    pub async fn get(&self, text: &str, from: &str, to: &str) -> Option<CachedTranslation> {
        let key = Self::make_key(text, from, to);

        // Conditional cleanup: only sweep expired entries once per CLEANUP_INTERVAL_MS
        {
            let now = Utc::now().timestamp_millis();
            let mut last = self.last_cleanup.lock().await;
            if now - *last > CLEANUP_INTERVAL_MS {
                *last = now;
                drop(last);
                let conn = self.conn.lock().await;
                let cutoff = now - (self.ttl_hours * 3600 * 1000);
                if let Err(e) = conn.execute(
                    "DELETE FROM translations WHERE timestamp < ?1",
                    params![cutoff],
                ) {
                    tracing::warn!("Failed to evict expired cache entries: {}", e);
                }
            }
        }

        let conn = self.conn.lock().await;

        // Single query for all cached results with metadata from first row
        let mut stmt = conn
            .prepare(
                "SELECT engine, translated_text, timestamp, hits
                 FROM translations
                 WHERE cache_key = ?1
                 ORDER BY engine",
            )
            .ok()?;

        let mut results: Vec<(String, String)> = Vec::new();
        let mut timestamp: i64 = 0;
        let mut hits: i64 = 0;

        let rows = stmt
            .query_map(params![key], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .ok()?;

        for (i, row) in rows.enumerate() {
            if let Ok((engine, translated, ts, h)) = row {
                if i == 0 {
                    timestamp = ts;
                    hits = h;
                }
                results.push((engine, translated));
            }
        }

        if results.is_empty() {
            return None;
        }

        // Increment hit count
        if let Err(e) = conn.execute(
            "UPDATE translations SET hits = hits + 1 WHERE cache_key = ?1",
            params![key],
        ) {
            tracing::warn!("Failed to increment cache hit count: {}", e);
        }

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
        if let Err(e) = conn.execute(
            "DELETE FROM translations WHERE cache_key = ?1",
            params![key],
        ) {
            tracing::error!("Failed to delete old cache entries for key: {}", e);
        }

        // Insert new results
        for (engine, translated) in &results {
            if let Err(e) = conn.execute(
                "INSERT INTO translations (cache_key, from_lang, to_lang, source_text, engine, translated_text, timestamp, hits)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                params![key, from, to, text, engine, translated, timestamp],
            ) {
                tracing::error!("Failed to insert cache entry (engine={}): {}", engine, e);
            }
        }

        // Evict oldest entries if cache exceeds max size
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT cache_key) FROM translations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count > self.max_size as i64 {
            let to_delete = count - self.max_size as i64;
            if let Err(e) = conn.execute(
                "DELETE FROM translations WHERE cache_key IN (
                    SELECT cache_key FROM translations
                    GROUP BY cache_key
                    ORDER BY MIN(timestamp) ASC
                    LIMIT ?1
                )",
                params![to_delete],
            ) {
                tracing::warn!("Failed to evict old cache entries: {}", e);
            }
        }
    }

    pub async fn clear(&self) {
        let conn = self.conn.lock().await;
        if let Err(e) = conn.execute("DELETE FROM translations", []) {
            tracing::error!("Failed to clear translation cache: {}", e);
        }
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
            .query_row("SELECT SUM(hits) FROM translations", [], |row| row.get(0))
            .unwrap_or(0);

        // Get per-engine stats
        let engine_stats = match conn
            .prepare("SELECT engine, COUNT(*), SUM(hits) FROM translations GROUP BY engine")
        {
            Ok(mut stmt) => match stmt.query_map([], |row| {
                Ok(EngineStats {
                    engine: row.get(0)?,
                    entries: row.get(1)?,
                    hits: row.get(2)?,
                })
            }) {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(e) => {
                    tracing::error!("Failed to query engine stats: {}", e);
                    Vec::new()
                }
            },
            Err(e) => {
                tracing::error!("Failed to prepare engine stats query: {}", e);
                Vec::new()
            }
        };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_key_format() {
        let key = TranslationCache::make_key("hello", "en", "zh");
        assert_eq!(key, "en|zh|hello");
    }

    #[test]
    fn test_make_key_with_special_chars() {
        let key = TranslationCache::make_key("hello world", "en", "zh");
        assert_eq!(key, "en|zh|hello world");
    }

    #[test]
    fn test_make_key_different_inputs() {
        let key1 = TranslationCache::make_key("hello", "en", "zh");
        let key2 = TranslationCache::make_key("hello", "ja", "zh");
        let key3 = TranslationCache::make_key("world", "en", "zh");
        assert_ne!(key1, key2);
        assert_ne!(key1, key3);
    }
}

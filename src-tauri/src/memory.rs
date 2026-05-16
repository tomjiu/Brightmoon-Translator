use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

// Re-export shared types from models
pub use crate::models::memory::{HistoryItem, TmMatch, WordBookItem};

pub struct HistoryStore {
    conn: Mutex<Connection>,
}

pub struct WordBookStore {
    conn: Mutex<Connection>,
}

fn db_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    std::fs::create_dir_all(&path).ok();
    path.push("history.db");
    path
}

impl HistoryStore {
    pub fn load() -> Self {
        let path = db_path();
        let conn = match Connection::open(&path) {
            Ok(conn) => conn,
            Err(e) => {
                log::error!("Failed to open history database: {}", e);
                Connection::open_in_memory().expect("Failed to create in-memory history")
            }
        };

        // Create table if not exists
        if let Err(e) = conn.execute(
            "CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                source_text TEXT NOT NULL,
                translated_text TEXT NOT NULL,
                from_lang TEXT NOT NULL,
                to_lang TEXT NOT NULL,
                engine TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            )",
            [],
        ) {
            log::error!("Failed to create history table: {}", e);
        }

        // Create index for faster queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC)",
            [],
        )
        .ok();

        // Create index for TM lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_source_text ON history(source_text)",
            [],
        )
        .ok();

        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn add(&self, source: &str, translated: &str, from: &str, to: &str, engine: &str) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "INSERT INTO history (id, source_text, translated_text, from_lang, to_lang, engine, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, source, translated, from, to, engine, timestamp],
        ).ok();

        // Keep only last 10000 records
        conn.execute(
            "DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY timestamp DESC LIMIT 10000)",
            [],
        ).ok();
    }

    pub fn get_all(&self) -> Vec<HistoryItem> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = match conn.prepare("SELECT id, source_text, translated_text, from_lang, to_lang, engine, timestamp FROM history ORDER BY timestamp DESC") {
            Ok(stmt) => stmt,
            Err(e) => {
                log::error!("Failed to prepare history query: {}", e);
                return Vec::new();
            }
        };

        let result = stmt.query_map([], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                source_text: row.get(1)?,
                translated_text: row.get(2)?,
                from: row.get(3)?,
                to: row.get(4)?,
                engine: row.get(5)?,
                timestamp: row.get(6)?,
            })
        });

        match result {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                log::error!("Failed to query history: {}", e);
                Vec::new()
            }
        }
    }

    pub fn clear(&self) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute("DELETE FROM history", []).ok();
    }

    pub fn remove(&self, id: &str) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])
            .ok();
    }

    pub fn batch_remove(&self, ids: &[String]) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        for id in ids {
            conn.execute("DELETE FROM history WHERE id = ?1", params![id])
                .ok();
        }
    }

    /// Find the best translation memory match for the given source text.
    /// Priority: exact match > prefix match > contains match.
    /// Returns None if no match above the threshold.
    pub fn fuzzy_match(
        &self,
        source: &str,
        from: &str,
        to: &str,
        threshold: f64,
    ) -> Option<TmMatch> {
        let normalized = source.trim().to_lowercase();
        if normalized.is_empty() || normalized.len() < 2 {
            return None;
        }

        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        // 1. Exact match (case-insensitive) — fastest, indexed
        if let Some(m) = self.query_exact(&conn, &normalized, from, to) {
            return Some(m);
        }

        // 2. Prefix match: stored entry is a prefix of the query
        if let Some(m) = self.query_prefix(&conn, &normalized, from, to, threshold) {
            return Some(m);
        }

        // 3. Contains match: query contains a stored entry (or vice versa)
        self.query_contains(&conn, &normalized, from, to, threshold)
    }

    fn query_exact(
        &self,
        conn: &Connection,
        normalized: &str,
        from: &str,
        to: &str,
    ) -> Option<TmMatch> {
        let mut stmt = conn
            .prepare(
                "SELECT source_text, translated_text, engine, timestamp
                 FROM history
                 WHERE LOWER(TRIM(source_text)) = ?1 AND from_lang = ?2 AND to_lang = ?3
                 ORDER BY timestamp DESC LIMIT 1",
            )
            .ok()?;
        stmt.query_row(params![normalized, from, to], |row| {
            Ok(TmMatch {
                source_text: row.get(0)?,
                translated_text: row.get(1)?,
                engine: row.get(2)?,
                timestamp: row.get(3)?,
                similarity: 1.0,
            })
        })
        .ok()
    }

    fn query_prefix(
        &self,
        conn: &Connection,
        normalized: &str,
        from: &str,
        to: &str,
        threshold: f64,
    ) -> Option<TmMatch> {
        // Find entries whose source_text is a prefix of the query
        let mut stmt = conn
            .prepare(
                "SELECT source_text, translated_text, engine, timestamp
                 FROM history
                 WHERE from_lang = ?1 AND to_lang = ?2
                   AND ?3 LIKE LOWER(TRIM(source_text)) || '%'
                   AND LENGTH(TRIM(source_text)) >= 3
                 ORDER BY LENGTH(source_text) DESC, timestamp DESC
                 LIMIT 5",
            )
            .ok()?;
        let candidates: Vec<TmMatch> = stmt
            .query_map(params![from, to, normalized], |row| {
                Ok(TmMatch {
                    source_text: row.get(0)?,
                    translated_text: row.get(1)?,
                    engine: row.get(2)?,
                    timestamp: row.get(3)?,
                    similarity: 0.0,
                })
            })
            .ok()?
            .filter_map(|r| r.ok())
            .filter_map(|mut m| {
                let sim = prefix_similarity(&m.source_text, normalized);
                if sim >= threshold {
                    m.similarity = sim;
                    Some(m)
                } else {
                    None
                }
            })
            .collect();
        candidates.into_iter().max_by(|a, b| {
            a.similarity
                .partial_cmp(&b.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    fn query_contains(
        &self,
        conn: &Connection,
        normalized: &str,
        from: &str,
        to: &str,
        threshold: f64,
    ) -> Option<TmMatch> {
        // Find entries contained within the query or containing the query
        // Escape LIKE special chars to prevent pattern injection
        let escaped = normalized
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{}%", escaped);
        let mut stmt = conn
            .prepare(
                "SELECT source_text, translated_text, engine, timestamp
                 FROM history
                 WHERE from_lang = ?1 AND to_lang = ?2
                   AND (LOWER(TRIM(source_text)) LIKE ?3 ESCAPE '\\' OR ?4 LIKE '%' || LOWER(TRIM(source_text)) || '%')
                   AND LENGTH(TRIM(source_text)) >= 3
                 ORDER BY LENGTH(source_text) DESC, timestamp DESC
                 LIMIT 10",
            )
            .ok()?;
        let candidates: Vec<TmMatch> = stmt
            .query_map(params![from, to, pattern, normalized], |row| {
                Ok(TmMatch {
                    source_text: row.get(0)?,
                    translated_text: row.get(1)?,
                    engine: row.get(2)?,
                    timestamp: row.get(3)?,
                    similarity: 0.0,
                })
            })
            .ok()?
            .filter_map(|r| r.ok())
            .filter_map(|mut m| {
                let sim = substring_similarity(&m.source_text, normalized);
                if sim >= threshold {
                    m.similarity = sim;
                    Some(m)
                } else {
                    None
                }
            })
            .collect();
        candidates.into_iter().max_by(|a, b| {
            a.similarity
                .partial_cmp(&b.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Similarity for prefix match: stored is prefix of query
fn prefix_similarity(stored: &str, query: &str) -> f64 {
    let stored_norm = stored.trim().to_lowercase();
    let query_norm = query.trim().to_lowercase();
    if stored_norm.is_empty() || query_norm.is_empty() {
        return 0.0;
    }
    // stored is prefix of query
    if query_norm.starts_with(&stored_norm) {
        stored_norm.len() as f64 / query_norm.len() as f64
    } else {
        0.0
    }
}

/// Similarity for substring match: how much overlap
fn substring_similarity(stored: &str, query: &str) -> f64 {
    let stored_norm = stored.trim().to_lowercase();
    let query_norm = query.trim().to_lowercase();
    if stored_norm.is_empty() || query_norm.is_empty() {
        return 0.0;
    }
    let stored_len = stored_norm.len();
    let query_len = query_norm.len();
    let max_len = stored_len.max(query_len) as f64;
    let min_len = stored_len.min(query_len) as f64;
    // If one contains the other, ratio of shorter to longer
    if query_norm.contains(&stored_norm) || stored_norm.contains(&query_norm) {
        min_len / max_len
    } else {
        0.0
    }
}

impl WordBookStore {
    pub fn load() -> Self {
        let path = db_path();
        let conn = match Connection::open(&path) {
            Ok(conn) => conn,
            Err(e) => {
                log::error!("Failed to open wordbook database: {}", e);
                Connection::open_in_memory().expect("Failed to create in-memory wordbook")
            }
        };

        if let Err(e) = conn.execute(
            "CREATE TABLE IF NOT EXISTS wordbook (
                id TEXT PRIMARY KEY,
                word TEXT NOT NULL,
                translation TEXT NOT NULL,
                from_lang TEXT NOT NULL,
                to_lang TEXT NOT NULL,
                note TEXT DEFAULT '',
                timestamp INTEGER NOT NULL
            )",
            [],
        ) {
            log::error!("Failed to create wordbook table: {}", e);
        }

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_wordbook_timestamp ON wordbook(timestamp DESC)",
            [],
        )
        .ok();

        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_wordbook_word ON wordbook(word, from_lang, to_lang)",
            [],
        ).ok();

        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn add(
        &self,
        word: &str,
        translation: &str,
        from_lang: &str,
        to_lang: &str,
        note: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "INSERT OR REPLACE INTO wordbook (id, word, translation, from_lang, to_lang, note, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, word, translation, from_lang, to_lang, note, timestamp],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn get_all(&self) -> Vec<WordBookItem> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = match conn.prepare("SELECT id, word, translation, from_lang, to_lang, note, timestamp FROM wordbook ORDER BY timestamp DESC") {
            Ok(stmt) => stmt,
            Err(e) => {
                log::error!("Failed to prepare wordbook query: {}", e);
                return Vec::new();
            }
        };

        let result = stmt.query_map([], |row| {
            Ok(WordBookItem {
                id: row.get(0)?,
                word: row.get(1)?,
                translation: row.get(2)?,
                from_lang: row.get(3)?,
                to_lang: row.get(4)?,
                note: row.get(5)?,
                timestamp: row.get(6)?,
            })
        });

        match result {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                log::error!("Failed to query wordbook: {}", e);
                Vec::new()
            }
        }
    }

    pub fn update_note(&self, id: &str, note: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "UPDATE wordbook SET note = ?1 WHERE id = ?2",
            params![note, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove(&self, id: &str) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute("DELETE FROM wordbook WHERE id = ?1", params![id])
            .ok();
    }

    pub fn batch_remove(&self, ids: &[String]) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        for id in ids {
            conn.execute("DELETE FROM wordbook WHERE id = ?1", params![id])
                .ok();
        }
    }

    pub fn clear(&self) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute("DELETE FROM wordbook", []).ok();
    }

    pub fn search(&self, query: &str) -> Vec<WordBookItem> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = match conn.prepare("SELECT id, word, translation, from_lang, to_lang, note, timestamp FROM wordbook WHERE word LIKE ?1 OR translation LIKE ?1 ORDER BY timestamp DESC") {
            Ok(stmt) => stmt,
            Err(e) => {
                log::error!("Failed to prepare wordbook search: {}", e);
                return Vec::new();
            }
        };

        let pattern = format!("%{}%", query);
        let result = stmt.query_map(params![pattern], |row| {
            Ok(WordBookItem {
                id: row.get(0)?,
                word: row.get(1)?,
                translation: row.get(2)?,
                from_lang: row.get(3)?,
                to_lang: row.get(4)?,
                note: row.get(5)?,
                timestamp: row.get(6)?,
            })
        });

        match result {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                log::error!("Failed to search wordbook: {}", e);
                Vec::new()
            }
        }
    }
}

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

// Re-export shared types from models
pub use crate::models::memory::{HistoryItem, TmMatch, WordBookItem};

/// TM Export/Import format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TmExportEntry {
    pub source: String,
    pub target: String,
    pub from_lang: String,
    pub to_lang: String,
    pub engine: String,
    pub timestamp: i64,
}

/// TM Export container
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TmExportData {
    pub version: u32,
    pub entries: Vec<TmExportEntry>,
    pub exported_at: i64,
}

pub struct HistoryStore {
    conn: Mutex<Connection>,
}

pub struct WordBookStore {
    conn: Mutex<Connection>,
}

fn db_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!("Failed to create history db directory {:?}: {}", path, e);
    }
    path.push("history.db");
    path
}

impl HistoryStore {
    pub fn load() -> Self {
        let path = db_path();
        let conn = match Connection::open(&path) {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("Failed to open history database: {}", e);
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
            tracing::error!("Failed to create history table: {}", e);
        }

        // Create index for faster queries
        if let Err(e) = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC)",
            [],
        ) {
            tracing::warn!("Failed to create history timestamp index: {}", e);
        }

        // Create index for TM lookups
        if let Err(e) = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_source_text ON history(source_text)",
            [],
        ) {
            tracing::warn!("Failed to create history source_text index: {}", e);
        }

        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn add(&self, source: &str, translated: &str, from: &str, to: &str, engine: &str) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp_millis();

        if let Err(e) = conn.execute(
            "INSERT INTO history (id, source_text, translated_text, from_lang, to_lang, engine, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, source, translated, from, to, engine, timestamp],
        ) {
            tracing::error!("Failed to insert history record: {}", e);
        }

        // Keep only last 10000 records
        if let Err(e) = conn.execute(
            "DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY timestamp DESC LIMIT 10000)",
            [],
        ) {
            tracing::warn!("Failed to evict old history records: {}", e);
        }
    }

    pub fn get_all(&self) -> Vec<HistoryItem> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = match conn.prepare("SELECT id, source_text, translated_text, from_lang, to_lang, engine, timestamp FROM history ORDER BY timestamp DESC") {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("Failed to prepare history query: {}", e);
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
                tracing::error!("Failed to query history: {}", e);
                Vec::new()
            }
        }
    }

    pub fn clear(&self) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = conn.execute("DELETE FROM history", []) {
            tracing::error!("Failed to clear history: {}", e);
        }
    }

    pub fn remove(&self, id: &str) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = conn.execute("DELETE FROM history WHERE id = ?1", params![id]) {
            tracing::error!("Failed to remove history item {}: {}", id, e);
        }
    }

    pub fn batch_remove(&self, ids: &[String]) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        for id in ids {
            if let Err(e) = conn.execute("DELETE FROM history WHERE id = ?1", params![id]) {
                tracing::error!("Failed to remove history item {}: {}", id, e);
            }
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

    /// Export all TM entries as JSON-serializable data.
    /// Uses parameterized queries to prevent SQL injection.
    pub fn export_tm(&self, from: Option<&str>, to: Option<&str>) -> TmExportData {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        // Build query with parameterized placeholders to prevent SQL injection
        let (query, param_values): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match (from, to) {
            (Some(f), Some(t)) => (
                "SELECT source_text, translated_text, from_lang, to_lang, engine, timestamp
                 FROM history WHERE from_lang = ?1 AND to_lang = ?2 ORDER BY timestamp DESC"
                    .to_string(),
                vec![Box::new(f.to_string()), Box::new(t.to_string())],
            ),
            (Some(f), None) => (
                "SELECT source_text, translated_text, from_lang, to_lang, engine, timestamp
                 FROM history WHERE from_lang = ?1 ORDER BY timestamp DESC"
                    .to_string(),
                vec![Box::new(f.to_string())],
            ),
            (None, Some(t)) => (
                "SELECT source_text, translated_text, from_lang, to_lang, engine, timestamp
                 FROM history WHERE to_lang = ?1 ORDER BY timestamp DESC"
                    .to_string(),
                vec![Box::new(t.to_string())],
            ),
            (None, None) => (
                "SELECT source_text, translated_text, from_lang, to_lang, engine, timestamp
                 FROM history ORDER BY timestamp DESC"
                    .to_string(),
                vec![],
            ),
        };

        let mut stmt = match conn.prepare(&query) {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("Failed to prepare TM export query: {}", e);
                return TmExportData {
                    version: 1,
                    entries: Vec::new(),
                    exported_at: chrono::Utc::now().timestamp_millis(),
                };
            }
        };

        let entries: Vec<TmExportEntry> = stmt
            .query_map(rusqlite::params_from_iter(param_values.iter().map(|p| p.as_ref())), |row| {
                Ok(TmExportEntry {
                    source: row.get(0)?,
                    target: row.get(1)?,
                    from_lang: row.get(2)?,
                    to_lang: row.get(3)?,
                    engine: row.get(4)?,
                    timestamp: row.get(5)?,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        TmExportData {
            version: 1,
            entries,
            exported_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Import TM entries from exported data
    /// Returns (imported_count, skipped_count)
    pub fn import_tm(&self, data: &TmExportData, deduplicate: bool) -> (usize, usize) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut imported = 0;
        let mut skipped = 0;

        // If deduplicating, load existing source texts for quick lookup
        let existing: HashSet<String> = if deduplicate {
            let mut stmt = match conn.prepare("SELECT DISTINCT LOWER(TRIM(source_text)) FROM history") {
                Ok(stmt) => stmt,
                Err(e) => {
                    tracing::error!("Failed to prepare dedup query: {}", e);
                    return (0, data.entries.len());
                }
            };
            stmt.query_map([], |row| row.get::<_, String>(0))
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        } else {
            HashSet::new()
        };

        for entry in &data.entries {
            let normalized = entry.source.trim().to_lowercase();
            if deduplicate && existing.contains(&normalized) {
                skipped += 1;
                continue;
            }

            let id = Uuid::new_v4().to_string();
            if let Err(e) = conn.execute(
                "INSERT INTO history (id, source_text, translated_text, from_lang, to_lang, engine, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, entry.source, entry.target, entry.from_lang, entry.to_lang, entry.engine, entry.timestamp],
            ) {
                tracing::warn!("Failed to import TM entry: {}", e);
                skipped += 1;
            } else {
                imported += 1;
            }
        }

        (imported, skipped)
    }

    /// Get TM statistics
    pub fn get_tm_stats(&self) -> TmStats {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let total = conn
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as usize;

        let lang_pairs: Vec<(String, String, usize)> = {
            let mut stmt = match conn.prepare(
                "SELECT from_lang, to_lang, COUNT(*) as cnt
                 FROM history GROUP BY from_lang, to_lang ORDER BY cnt DESC",
            ) {
                Ok(stmt) => stmt,
                Err(_) => return TmStats { total, lang_pairs: Vec::new() },
            };
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, usize>(2)?,
                ))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        };

        TmStats { total, lang_pairs }
    }

    /// Search TM entries by query text and optional language pair filter.
    /// Uses parameterized queries and escaped LIKE patterns to prevent injection.
    pub fn search_tm(
        &self,
        query: &str,
        from_lang: Option<&str>,
        to_lang: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> (Vec<TmExportEntry>, usize) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        // Escape LIKE special characters to prevent pattern injection
        let escaped_query = query.to_lowercase()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let query_pattern = format!("%{}%", escaped_query);

        let mut conditions = vec![
            "(LOWER(source_text) LIKE ?1 ESCAPE '\\' OR LOWER(translated_text) LIKE ?1 ESCAPE '\\')".to_string(),
        ];
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(query_pattern.clone()),
        ];

        let mut param_idx = 2;
        if let Some(from) = from_lang {
            conditions.push(format!("from_lang = ?{}", param_idx));
            params.push(Box::new(from.to_string()));
            param_idx += 1;
        }
        if let Some(to) = to_lang {
            conditions.push(format!("to_lang = ?{}", param_idx));
            params.push(Box::new(to.to_string()));
            param_idx += 1;
        }

        let where_clause = conditions.join(" AND ");

        // Get total count
        let count_query = format!(
            "SELECT COUNT(*) FROM history WHERE {}",
            where_clause
        );
        let total: usize = conn
            .query_row(
                &count_query,
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0) as usize;

        // Get paginated results
        let query_str = format!(
            "SELECT source_text, translated_text, from_lang, to_lang, engine, timestamp
             FROM history WHERE {} ORDER BY timestamp DESC LIMIT ?{} OFFSET ?{}",
            where_clause, param_idx, param_idx + 1
        );
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let entries: Vec<TmExportEntry> = conn
            .prepare(&query_str)
            .ok()
            .map(|mut stmt| {
                stmt.query_map(
                    rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                    |row| {
                        Ok(TmExportEntry {
                            source: row.get(0)?,
                            target: row.get(1)?,
                            from_lang: row.get(2)?,
                            to_lang: row.get(3)?,
                            engine: row.get(4)?,
                            timestamp: row.get(5)?,
                        })
                    },
                )
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
            })
            .unwrap_or_default();

        (entries, total)
    }

    /// Delete TM entries matching the given criteria.
    /// Returns the number of entries deleted.
    pub fn delete_tm(
        &self,
        source: &str,
        target: &str,
        from_lang: &str,
        to_lang: &str,
    ) -> usize {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "DELETE FROM history WHERE source_text = ?1 AND translated_text = ?2 AND from_lang = ?3 AND to_lang = ?4",
            params![source, target, from_lang, to_lang],
        )
        .unwrap_or(0)
    }

    /// Bulk delete TM entries by a list of source/target/lang tuples.
    /// Returns the total number of entries deleted.
    pub fn batch_delete_tm(
        &self,
        entries: &[(String, String, String, String)],
    ) -> usize {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut total_deleted = 0usize;
        for (source, target, from_lang, to_lang) in entries {
            total_deleted += conn
                .execute(
                    "DELETE FROM history WHERE source_text = ?1 AND translated_text = ?2 AND from_lang = ?3 AND to_lang = ?4",
                    params![source, target, from_lang, to_lang],
                )
                .unwrap_or(0);
        }
        total_deleted
    }
}

/// TM statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TmStats {
    pub total: usize,
    pub lang_pairs: Vec<(String, String, usize)>,
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
                tracing::error!("Failed to open wordbook database: {}", e);
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
            tracing::error!("Failed to create wordbook table: {}", e);
        }

        if let Err(e) = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_wordbook_timestamp ON wordbook(timestamp DESC)",
            [],
        ) {
            tracing::warn!("Failed to create wordbook timestamp index: {}", e);
        }

        if let Err(e) = conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_wordbook_word ON wordbook(word, from_lang, to_lang)",
            [],
        ) {
            tracing::warn!("Failed to create wordbook word index: {}", e);
        }

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
                tracing::error!("Failed to prepare wordbook query: {}", e);
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
                tracing::error!("Failed to query wordbook: {}", e);
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
        if let Err(e) = conn.execute("DELETE FROM wordbook WHERE id = ?1", params![id]) {
            tracing::error!("Failed to remove wordbook item {}: {}", id, e);
        }
    }

    pub fn batch_remove(&self, ids: &[String]) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        for id in ids {
            if let Err(e) = conn.execute("DELETE FROM wordbook WHERE id = ?1", params![id]) {
                tracing::error!("Failed to remove wordbook item {}: {}", id, e);
            }
        }
    }

    pub fn clear(&self) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = conn.execute("DELETE FROM wordbook", []) {
            tracing::error!("Failed to clear wordbook: {}", e);
        }
    }

    /// Search wordbook entries by query.
    /// Uses escaped LIKE patterns to prevent pattern injection.
    pub fn search(&self, query: &str) -> Vec<WordBookItem> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = match conn.prepare("SELECT id, word, translation, from_lang, to_lang, note, timestamp FROM wordbook WHERE word LIKE ?1 ESCAPE '\\' OR translation LIKE ?1 ESCAPE '\\' ORDER BY timestamp DESC") {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("Failed to prepare wordbook search: {}", e);
                return Vec::new();
            }
        };

        // Escape LIKE special characters to prevent pattern injection
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{}%", escaped);
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
                tracing::error!("Failed to search wordbook: {}", e);
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_similarity_exact_match() {
        let sim = prefix_similarity("hello", "hello");
        assert_eq!(sim, 1.0);
    }

    #[test]
    fn test_prefix_similarity_prefix_match() {
        let sim = prefix_similarity("hel", "hello");
        assert!((sim - 0.6).abs() < 0.001); // 3/5 = 0.6
    }

    #[test]
    fn test_prefix_similarity_no_match() {
        let sim = prefix_similarity("world", "hello");
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_prefix_similarity_empty_strings() {
        assert_eq!(prefix_similarity("", "hello"), 0.0);
        assert_eq!(prefix_similarity("hello", ""), 0.0);
    }

    #[test]
    fn test_prefix_similarity_case_insensitive() {
        let sim = prefix_similarity("HELLO", "hello world");
        assert!((sim - 5.0 / 11.0).abs() < 0.001);
    }

    #[test]
    fn test_substring_similarity_exact_match() {
        let sim = substring_similarity("hello", "hello");
        assert_eq!(sim, 1.0);
    }

    #[test]
    fn test_substring_similarity_contained() {
        let sim = substring_similarity("ell", "hello");
        assert!((sim - 3.0 / 5.0).abs() < 0.001); // shorter/longer
    }

    #[test]
    fn test_substring_similarity_contains() {
        let sim = substring_similarity("hello", "ell");
        assert!((sim - 3.0 / 5.0).abs() < 0.001); // shorter/longer
    }

    #[test]
    fn test_substring_similarity_no_overlap() {
        let sim = substring_similarity("abc", "xyz");
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_substring_similarity_empty_strings() {
        assert_eq!(substring_similarity("", "hello"), 0.0);
        assert_eq!(substring_similarity("hello", ""), 0.0);
    }
}

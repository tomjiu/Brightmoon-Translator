use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub source_text: String,
    pub translated_text: String,
    pub from: String,
    pub to: String,
    pub engine: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordBookItem {
    pub id: String,
    pub word: String,
    pub translation: String,
    pub from_lang: String,
    pub to_lang: String,
    pub note: String,
    pub timestamp: i64,
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
    std::fs::create_dir_all(&path).ok();
    path.push("history.db");
    path
}

impl HistoryStore {
    pub fn load() -> Self {
        let path = db_path();
        let conn = Connection::open(&path).expect("Failed to open SQLite database");

        // Create table if not exists
        conn.execute(
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
        ).expect("Failed to create history table");

        // Create index for faster queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC)",
            [],
        ).ok();

        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn add(&self, source: &str, translated: &str, from: &str, to: &str, engine: &str) {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, source_text, translated_text, from_lang, to_lang, engine, timestamp FROM history ORDER BY timestamp DESC")
            .unwrap();

        let items = stmt
            .query_map([], |row| {
                Ok(HistoryItem {
                    id: row.get(0)?,
                    source_text: row.get(1)?,
                    translated_text: row.get(2)?,
                    from: row.get(3)?,
                    to: row.get(4)?,
                    engine: row.get(5)?,
                    timestamp: row.get(6)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        items
    }

    pub fn clear(&self) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history", []).ok();
    }

    pub fn remove(&self, id: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history WHERE id = ?1", params![id]).ok();
    }

    pub fn batch_remove(&self, ids: &[String]) {
        let conn = self.conn.lock().unwrap();
        for id in ids {
            conn.execute("DELETE FROM history WHERE id = ?1", params![id]).ok();
        }
    }
}

impl WordBookStore {
    pub fn load() -> Self {
        let path = db_path();
        let conn = Connection::open(&path).expect("Failed to open SQLite database");

        conn.execute(
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
        ).expect("Failed to create wordbook table");

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_wordbook_timestamp ON wordbook(timestamp DESC)",
            [],
        ).ok();

        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_wordbook_word ON wordbook(word, from_lang, to_lang)",
            [],
        ).ok();

        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn add(&self, word: &str, translation: &str, from_lang: &str, to_lang: &str, note: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "INSERT OR REPLACE INTO wordbook (id, word, translation, from_lang, to_lang, note, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, word, translation, from_lang, to_lang, note, timestamp],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn get_all(&self) -> Vec<WordBookItem> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, word, translation, from_lang, to_lang, note, timestamp FROM wordbook ORDER BY timestamp DESC")
            .unwrap();

        stmt.query_map([], |row| {
            Ok(WordBookItem {
                id: row.get(0)?,
                word: row.get(1)?,
                translation: row.get(2)?,
                from_lang: row.get(3)?,
                to_lang: row.get(4)?,
                note: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn update_note(&self, id: &str, note: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE wordbook SET note = ?1 WHERE id = ?2",
            params![note, id],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove(&self, id: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM wordbook WHERE id = ?1", params![id]).ok();
    }

    pub fn batch_remove(&self, ids: &[String]) {
        let conn = self.conn.lock().unwrap();
        for id in ids {
            conn.execute("DELETE FROM wordbook WHERE id = ?1", params![id]).ok();
        }
    }

    pub fn clear(&self) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM wordbook", []).ok();
    }

    pub fn search(&self, query: &str) -> Vec<WordBookItem> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, word, translation, from_lang, to_lang, note, timestamp FROM wordbook WHERE word LIKE ?1 OR translation LIKE ?1 ORDER BY timestamp DESC")
            .unwrap();

        let pattern = format!("%{}%", query);
        stmt.query_map(params![pattern], |row| {
            Ok(WordBookItem {
                id: row.get(0)?,
                word: row.get(1)?,
                translation: row.get(2)?,
                from_lang: row.get(3)?,
                to_lang: row.get(4)?,
                note: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }
}

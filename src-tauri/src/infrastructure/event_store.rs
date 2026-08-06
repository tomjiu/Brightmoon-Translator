// Event Store - 事件持久化层（使用 sqlx）

use crate::domain::{CardEvent, WordCard};
use anyhow::{Context, Result};
use sqlx::{Row, SqlitePool};

/// Event Store - 事件存储
#[derive(Clone)]
pub struct EventStore {
    pool: SqlitePool,
}

impl EventStore {
    /// 创建 Event Store
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url)
            .await
            .context("Failed to connect to database")?;

        Ok(Self { pool })
    }

    /// 从连接池创建
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 获取数据库连接池（供其他模块直接查询）
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// 初始化数据库 schema
    pub async fn init_schema(&self) -> Result<()> {
        // 事件流表（唯一真相）
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS card_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                event_data TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 索引
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_card_events_card_id
            ON card_events(card_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_card_events_timestamp
            ON card_events(timestamp)
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 卡牌快照表（可选，性能优化）
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS cards (
                id TEXT PRIMARY KEY,
                word TEXT NOT NULL,
                current_version INTEGER NOT NULL DEFAULT 1,
                ai_content TEXT,
                fsrs_state TEXT,
                learning_state TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_cards_word
            ON cards(word)
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Patch 历史表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS card_patches (
                id TEXT PRIMARY KEY,
                card_id TEXT NOT NULL,
                version INTEGER NOT NULL,
                target_field TEXT NOT NULL,
                operation TEXT NOT NULL,
                old_value TEXT,
                new_value TEXT,
                reasoning TEXT,
                confidence REAL,
                generated_by TEXT,
                applied_at INTEGER,
                FOREIGN KEY (card_id) REFERENCES cards(id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_patches_card_version
            ON card_patches(card_id, version)
            "#,
        )
        .execute(&self.pool)
        .await?;

        // FTS5 全文搜索索引（word + ai_content 可搜索）
        let fts_result = sqlx::query(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS cards_fts
            USING fts5(word, ai_content, content='cards', content_rowid='rowid')
            "#,
        )
        .execute(&self.pool)
        .await;

        if fts_result.is_ok() {
            // 触发cards 表写入同步更新 FTS 索引
            for trigger in [
                // 插入同步
                r#"
                CREATE TRIGGER IF NOT EXISTS trg_cards_fts_insert
                AFTER INSERT ON cards BEGIN
                    INSERT INTO cards_fts(rowid, word, ai_content)
                    VALUES (new.rowid, new.word, new.ai_content);
                END
                "#,
                // 删除同步
                r#"
                CREATE TRIGGER IF NOT EXISTS trg_cards_fts_delete
                AFTER DELETE ON cards BEGIN
                    INSERT INTO cards_fts(cards_fts, rowid, word, ai_content)
                    VALUES ('delete', old.rowid, old.word, old.ai_content);
                END
                "#,
                // 更新同步
                r#"
                CREATE TRIGGER IF NOT EXISTS trg_cards_fts_update
                AFTER UPDATE ON cards BEGIN
                    INSERT INTO cards_fts(cards_fts, rowid, word, ai_content)
                    VALUES ('delete', old.rowid, old.word, old.ai_content);
                    INSERT INTO cards_fts(rowid, word, ai_content)
                    VALUES (new.rowid, new.word, new.ai_content);
                END
                "#,
            ] {
                sqlx::query(trigger).execute(&self.pool).await?;
            }
        } else {
            tracing::warn!("FTS5 不可用，跳过全文搜索索引（搜索将退化为 LIKE 查询）");
        }

        println!("✅ Event Store schema initialized");
        Ok(())
    }

    /// 全文搜索卡牌（FTS5，退化为 LIKE 兜底）
    pub async fn search_cards(&self, query: &str, limit: i64) -> Result<Vec<WordCard>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // 尝试 FTS5 精确匹配
        let fts_rows = sqlx::query(
            r#"
            SELECT c.id, c.word, c.current_version, c.ai_content, c.fsrs_state,
                   c.created_at, c.updated_at,
                   bm25(cards_fts) as rank
            FROM cards_fts
            JOIN cards c ON c.rowid = cards_fts.rowid
            WHERE cards_fts MATCH ?1
            ORDER BY rank ASC
            LIMIT ?2
            "#,
        )
        .bind(&query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await;

        let rows = match fts_rows {
            Ok(rows) if !rows.is_empty() => rows,
            _ => {
                // FTS5 语法错误或不可用时退化为 LIKE 搜索
                let like = format!("%{}%", query);
                sqlx::query(
                    r#"
                    SELECT id, word, current_version, ai_content, fsrs_state, created_at, updated_at
                    FROM cards
                    WHERE word LIKE ?1 OR ai_content LIKE ?1
                    ORDER BY CASE WHEN word = ?1 THEN 0 WHEN word LIKE ?2 THEN 1 ELSE 2 END, updated_at DESC
                    LIMIT ?3
                    "#,
                )
                .bind(&like)
                .bind(&query)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
        };

        let mut cards = Vec::new();
        for row in rows {
            let ai_content_json: String = row.try_get("ai_content")?;
            let fsrs_state_json: String = row.try_get("fsrs_state")?;
            cards.push(WordCard {
                id: row.try_get("id")?,
                word: row.try_get("word")?,
                current_version: row.try_get::<i64, _>("current_version")? as u32,
                base_data: crate::domain::BaseData {
                    phonetic: None,
                    part_of_speech: None,
                    definitions: Vec::new(),
                    translation: None,
                },
                ai_content: serde_json::from_str(&ai_content_json)?,
                fsrs_state: serde_json::from_str(&fsrs_state_json)?,
                error_records: Vec::new(),
                annotations: Vec::new(),
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }

        Ok(cards)
    }

    /// 追加事件（Event Sourcing 核心操作）
    pub async fn append_event(&self, card_id: &str, event: &CardEvent) -> Result<i64> {
        let event_type = event.event_type();
        let event_data = serde_json::to_string(event)?;
        let timestamp = event.timestamp();

        let result = sqlx::query(
            r#"
            INSERT INTO card_events (card_id, event_type, event_data, timestamp)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(card_id)
        .bind(event_type)
        .bind(&event_data)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// 加载卡牌的所有事件
    pub async fn load_events(&self, card_id: &str) -> Result<Vec<CardEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT event_data FROM card_events
            WHERE card_id = ?
            ORDER BY timestamp ASC, id ASC
            "#,
        )
        .bind(card_id)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            let event_data: String = row.try_get("event_data")?;
            let event: CardEvent =
                serde_json::from_str(&event_data).context("Failed to deserialize event")?;
            events.push(event);
        }

        Ok(events)
    }

    /// 加载指定时间之前的事件（用于时间旅行）
    pub async fn load_events_before(
        &self,
        card_id: &str,
        before_timestamp: i64,
    ) -> Result<Vec<CardEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT event_data FROM card_events
            WHERE card_id = ? AND timestamp <= ?
            ORDER BY timestamp ASC, id ASC
            "#,
        )
        .bind(card_id)
        .bind(before_timestamp)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            let event_data: String = row.try_get("event_data")?;
            let event: CardEvent = serde_json::from_str(&event_data)?;
            events.push(event);
        }

        Ok(events)
    }

    /// 重建卡牌（从事件流）
    pub async fn rebuild_card(&self, card_id: &str) -> Result<WordCard> {
        let events = self.load_events(card_id).await?;
        WordCard::from_events(&events)
    }

    /// 获取卡牌在指定时间点的状态（时间旅行）
    pub async fn get_card_at_time(&self, card_id: &str, timestamp: i64) -> Result<WordCard> {
        let events = self.load_events_before(card_id, timestamp).await?;
        WordCard::from_events(&events)
    }

    /// 更新卡牌快照（性能优化）
    pub async fn update_snapshot(&self, card: &WordCard) -> Result<()> {
        let ai_content_json = serde_json::to_string(&card.ai_content)?;
        let fsrs_state_json = serde_json::to_string(&card.fsrs_state)?;

        sqlx::query(
            r#"
            INSERT INTO cards (id, word, current_version, ai_content, fsrs_state, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                word = excluded.word,
                current_version = excluded.current_version,
                ai_content = excluded.ai_content,
                fsrs_state = excluded.fsrs_state,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&card.id)
        .bind(&card.word)
        .bind(card.current_version as i64)
        .bind(&ai_content_json)
        .bind(&fsrs_state_json)
        .bind(card.created_at)
        .bind(card.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 从快照加载卡牌（快速查询）
    pub async fn load_snapshot(&self, card_id: &str) -> Result<Option<WordCard>> {
        let row = sqlx::query(
            r#"
            SELECT id, word, current_version, ai_content, fsrs_state, created_at, updated_at
            FROM cards
            WHERE id = ?
            "#,
        )
        .bind(card_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let ai_content_json: String = row.try_get("ai_content")?;
            let fsrs_state_json: String = row.try_get("fsrs_state")?;

            let card = WordCard {
                id: row.try_get("id")?,
                word: row.try_get("word")?,
                current_version: row.try_get::<i64, _>("current_version")? as u32,
                base_data: crate::domain::BaseData {
                    phonetic: None,
                    part_of_speech: None,
                    definitions: Vec::new(),
                    translation: None,
                },
                ai_content: serde_json::from_str(&ai_content_json)?,
                fsrs_state: serde_json::from_str(&fsrs_state_json)?,
                error_records: Vec::new(),
                annotations: Vec::new(),
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };

            Ok(Some(card))
        } else {
            Ok(None)
        }
    }

    /// 获取所有卡牌 ID
    pub async fn get_all_card_ids(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT card_id FROM card_events
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let ids: Vec<String> = rows
            .into_iter()
            .map(|row| row.try_get("card_id"))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids)
    }

    /// 统计事件数量
    pub async fn count_events(&self, card_id: &str) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM card_events WHERE card_id = ?
            "#,
        )
        .bind(card_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.try_get("count")?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CardEvent;

    #[tokio::test]
    async fn test_event_store_basic() {
        // 使用内存数据库测试
        let store = EventStore::new("sqlite::memory:").await.unwrap();
        store.init_schema().await.unwrap();

        let card_id = "test-card-1";
        let event = CardEvent::WordImported {
            word: "test".to_string(),
            source: "manual".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        // 追加事件
        store.append_event(card_id, &event).await.unwrap();

        // 加载事件
        let events = store.load_events(card_id).await.unwrap();
        assert_eq!(events.len(), 1);

        // 统计事件
        let count = store.count_events(card_id).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_fts5_search() {
        let store = EventStore::new("sqlite::memory:").await.unwrap();
        store.init_schema().await.unwrap();

        fn make_card(id: &str, word: &str, tip: &str) -> crate::domain::WordCard {
            crate::domain::WordCard {
                id: id.into(),
                word: word.into(),
                current_version: 1,
                base_data: crate::domain::BaseData {
                    phonetic: None,
                    part_of_speech: None,
                    definitions: Vec::new(),
                    translation: None,
                },
                ai_content: Some(crate::domain::AiContent {
                    etymology: None,
                    mnemonics: vec![],
                    examples: vec![],
                    scenes: vec![],
                    collocations: vec![],
                    word_family: vec![],
                    usage_tips: vec![tip.into()],
                    common_mistakes: vec![],
                    synonyms: vec![],
                    antonyms: vec![],
                }),
                fsrs_state: crate::domain::CardState::default(),
                error_records: vec![],
                annotations: vec![],
                created_at: 0,
                updated_at: 0,
            }
        }

        store
            .update_snapshot(&make_card("c1", "serendipity", "幸运的意外发现"))
            .await
            .unwrap();
        store
            .update_snapshot(&make_card("c2", "apple", "苹果"))
            .await
            .unwrap();

        // 用 FTS5 搜索
        let results = store.search_cards("serendipity", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].word, "serendipity");

        // 退化 LIKE 搜索
        let results = store.search_cards("app", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].word, "apple");
    }
}

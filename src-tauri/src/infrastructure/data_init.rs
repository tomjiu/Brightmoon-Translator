// 数据初始化工具 - 导入词典数据

use anyhow::Result;
use sqlx::{Row, SqlitePool};

/// 数据初始化器
pub struct DataInitializer {
    pool: SqlitePool,
}

impl DataInitializer {
    /// 创建初始化器
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 执行初始化
    pub async fn initialize(&self) -> Result<()> {
        println!("🚀 开始数据初始化...\n");

        // 1. 创建 Schema
        self.create_schema().await?;

        // 2. 导入核心词库
        self.import_core_vocabulary().await?;

        // 3. 导入词根数据
        self.import_morphology().await?;

        // 4. 创建索引
        self.create_indexes().await?;

        println!("\n✅ 数据初始化完成！");
        Ok(())
    }

    /// 创建数据库 Schema
    async fn create_schema(&self) -> Result<()> {
        println!("1️⃣ 创建数据库 Schema");

        let schema = include_str!("../../migrations/001_initial_schema.sql");
        sqlx::raw_sql(schema).execute(&self.pool).await?;

        println!("   ✅ Schema 创建完成\n");
        Ok(())
    }

    /// 导入核心词库（从 ECDICT 筛选高频词）
    async fn import_core_vocabulary(&self) -> Result<()> {
        println!("2️⃣ 导入核心词库");

        // 连接 ECDICT 数据库
        let ecdict_pool = SqlitePool::connect("sqlite:../dictionaries/ecdict.db").await?;

        // 查询高频词（按词频排序，取前15000）
        let rows = sqlx::query(
            r#"
            SELECT word, frq, bnc, collins, oxford, tag
            FROM stardict
            WHERE frq IS NOT NULL
            ORDER BY frq DESC
            LIMIT 15000
            "#,
        )
        .fetch_all(&ecdict_pool)
        .await?;

        println!("   📊 从 ECDICT 查询到 {} 个词", rows.len());

        // 批量插入
        let mut tx = self.pool.begin().await?;

        for (rank, row) in rows.iter().enumerate() {
            let word: String = row.try_get("word")?;
            let frq: Option<i64> = row.try_get("frq").ok();
            let bnc: Option<i64> = row.try_get("bnc").ok();
            let collins: Option<i64> = row.try_get("collins").ok();
            let oxford: Option<i64> = row.try_get("oxford").ok();
            let tag: Option<String> = row.try_get("tag").ok();

            sqlx::query(
                r#"
                INSERT OR REPLACE INTO core_vocabulary
                (word, frequency_rank, frq, bnc, collins, oxford, tag)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&word)
            .bind((rank + 1) as i64)
            .bind(frq)
            .bind(bnc)
            .bind(collins)
            .bind(oxford)
            .bind(tag)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        // 更新统计
        sqlx::query(
            r#"
            UPDATE system_config
            SET value = ?, updated_at = strftime('%s', 'now')
            WHERE key = 'core_vocab_count'
            "#,
        )
        .bind(rows.len() as i64)
        .execute(&self.pool)
        .await?;

        println!("   ✅ 核心词库导入完成：{} 个词\n", rows.len());
        Ok(())
    }

    /// 导入词根数据（从 MorphoLex CSV）
    async fn import_morphology(&self) -> Result<()> {
        println!("3️⃣ 导入词根数据");

        use tokio::fs::File;
        use tokio::io::{AsyncBufReadExt, BufReader};

        let file = File::open("../dictionaries/morpholex/MorphoLEX_en.csv").await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // 跳过表头
        lines.next_line().await?;

        let mut count = 0;
        let mut tx = self.pool.begin().await?;

        while let Some(line) = lines.next_line().await? {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                let word = parts[0].trim().to_lowercase();
                let segmentation = parts[1].trim();
                let pos = if parts.len() >= 3 {
                    Some(parts[2].trim())
                } else {
                    None
                };

                // 解析 segmentation 为 parts JSON
                let parts_json = self.parse_segmentation(segmentation);

                sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO morphology
                    (word, segmentation, pos, parts)
                    VALUES (?, ?, ?, ?)
                    "#,
                )
                .bind(&word)
                .bind(segmentation)
                .bind(pos)
                .bind(&parts_json)
                .execute(&mut *tx)
                .await?;

                count += 1;

                if count % 5000 == 0 {
                    tx.commit().await?;
                    println!("   📝 已导入 {} 个词根", count);
                    tx = self.pool.begin().await?;
                }
            }
        }

        tx.commit().await?;

        // 更新统计
        sqlx::query(
            r#"
            UPDATE system_config
            SET value = ?, updated_at = strftime('%s', 'now')
            WHERE key = 'morphology_count'
            "#,
        )
        .bind(count as i64)
        .execute(&self.pool)
        .await?;

        println!("   ✅ 词根数据导入完成：{} 个词\n", count);
        Ok(())
    }

    /// 解析 segmentation 为 JSON
    fn parse_segmentation(&self, segmentation: &str) -> String {
        let parts: Vec<_> = segmentation
            .split('.')
            .enumerate()
            .map(|(i, part)| {
                let part_type = if i == 0 && segmentation.contains('.') {
                    "prefix"
                } else if part.ends_with('-') || part.starts_with('-') {
                    if i == 0 {
                        "prefix"
                    } else {
                        "suffix"
                    }
                } else {
                    "root"
                };

                serde_json::json!({
                    "part": part,
                    "part_type": part_type,
                    "meaning": null
                })
            })
            .collect();

        serde_json::to_string(&parts).unwrap()
    }

    /// 创建索引
    async fn create_indexes(&self) -> Result<()> {
        println!("4️⃣ 创建索引");

        // 所有索引已在 Schema 中定义
        println!("   ✅ 索引已创建\n");
        Ok(())
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> Result<InitStats> {
        let rows = sqlx::query(
            r#"
            SELECT key, value FROM system_config
            WHERE key IN ('core_vocab_count', 'morphology_count', 'etymology_count')
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut stats = InitStats::default();
        for row in rows {
            let key: String = row.try_get("key")?;
            let value: String = row.try_get("value")?;
            let value_num = value.parse::<i64>().unwrap_or(0);

            match key.as_str() {
                "core_vocab_count" => stats.core_vocab_count = value_num,
                "morphology_count" => stats.morphology_count = value_num,
                "etymology_count" => stats.etymology_count = value_num,
                _ => {},
            }
        }

        Ok(stats)
    }
}

/// 初始化统计
#[derive(Debug, Default)]
pub struct InitStats {
    pub core_vocab_count: i64,
    pub morphology_count: i64,
    pub etymology_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要真实数据库
    async fn test_parse_segmentation() {
        let initializer =
            DataInitializer::new(SqlitePool::connect("sqlite::memory:").await.unwrap());

        let result = initializer.parse_segmentation("brill.i.ant");
        assert!(result.contains("brill"));
        assert!(result.contains("part_type"));
    }
}

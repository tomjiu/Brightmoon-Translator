// Dictionary Skill - ECDICT 词典查询

use super::{Skill, SkillInput, SkillOutput};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

/// 词典查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub word: String,
    pub phonetic: Option<String>,
    pub definition: Option<String>,
    pub translation: Option<String>,
    pub pos: Option<String>,
    pub collins: Option<i32>,
    pub oxford: Option<i32>,
    pub tag: Option<String>,
    pub bnc: Option<i32>,
    pub frq: Option<i32>,
    pub exchange: Option<String>,
}

/// Dictionary Skill - 词典查询
pub struct DictionarySkill {
    pool: SqlitePool,
}

impl DictionarySkill {
    /// 创建 DictionarySkill
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 查询单词
    async fn lookup_word(&self, word: &str) -> Result<Option<DictionaryEntry>> {
        let result = sqlx::query(
            r#"
            SELECT
                word,
                phonetic,
                definition,
                translation,
                pos,
                collins,
                oxford,
                tag,
                bnc,
                frq,
                exchange
            FROM stardict
            WHERE word = ?
            LIMIT 1
            "#,
        )
        .bind(word)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = result {
            Ok(Some(DictionaryEntry {
                word: row.try_get("word")?,
                phonetic: row.try_get("phonetic").ok(),
                definition: row.try_get("definition").ok(),
                translation: row.try_get("translation").ok(),
                pos: row.try_get("pos").ok(),
                collins: row.try_get("collins").ok(),
                oxford: row.try_get("oxford").ok(),
                tag: row.try_get("tag").ok(),
                bnc: row.try_get("bnc").ok(),
                frq: row.try_get("frq").ok(),
                exchange: row.try_get("exchange").ok(),
            }))
        } else {
            Ok(None)
        }
    }

    /// 模糊搜索
    async fn search_words(&self, pattern: &str, limit: i32) -> Result<Vec<DictionaryEntry>> {
        let search_pattern = format!("%{}%", pattern);

        let rows = sqlx::query(
            r#"
            SELECT
                word,
                phonetic,
                definition,
                translation,
                pos,
                collins,
                oxford,
                tag,
                bnc,
                frq,
                exchange
            FROM stardict
            WHERE word LIKE ?
            ORDER BY frq DESC
            LIMIT ?
            "#,
        )
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(DictionaryEntry {
                word: row.try_get("word")?,
                phonetic: row.try_get("phonetic").ok(),
                definition: row.try_get("definition").ok(),
                translation: row.try_get("translation").ok(),
                pos: row.try_get("pos").ok(),
                collins: row.try_get("collins").ok(),
                oxford: row.try_get("oxford").ok(),
                tag: row.try_get("tag").ok(),
                bnc: row.try_get("bnc").ok(),
                frq: row.try_get("frq").ok(),
                exchange: row.try_get("exchange").ok(),
            });
        }

        Ok(results)
    }
}

#[async_trait]
impl Skill for DictionarySkill {
    fn name(&self) -> &str {
        "dictionary"
    }

    fn description(&self) -> &str {
        "查询 ECDICT 词典，支持精确查询和模糊搜索"
    }

    async fn execute(&self, input: SkillInput) -> Result<SkillOutput> {
        let word = input.primary.to_lowercase();

        // 检查是否是模糊搜索
        let is_search = input
            .get_param("search")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let limit = input
            .get_param("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(10) as i32;

        if is_search {
            // 模糊搜索
            let results = self.search_words(&word, limit).await?;
            Ok(SkillOutput::from_json(&results)?
                .with_metadata("source", serde_json::json!("ecdict"))
                .with_metadata("search", serde_json::json!(true))
                .with_metadata("count", serde_json::json!(results.len())))
        } else {
            // 精确查询
            let result = self.lookup_word(&word).await?;

            if let Some(entry) = result {
                Ok(SkillOutput::from_json(&entry)?
                    .with_metadata("source", serde_json::json!("ecdict"))
                    .with_metadata("found", serde_json::json!(true)))
            } else {
                Ok(SkillOutput::new(serde_json::json!(null))
                    .with_metadata("source", serde_json::json!("ecdict"))
                    .with_metadata("found", serde_json::json!(false)))
            }
        }
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        if input.primary.is_empty() {
            anyhow::bail!("单词不能为空");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要真实数据库
    async fn test_dictionary_lookup() {
        let pool = SqlitePool::connect("sqlite:../dictionaries/ecdict.db")
            .await
            .unwrap();

        let skill = DictionarySkill::new(pool);

        let input = SkillInput::new("hello");
        let output = skill.execute(input).await.unwrap();

        assert_eq!(output.metadata["found"], true);

        let entry: DictionaryEntry = output.into_type().unwrap();
        assert_eq!(entry.word, "hello");
    }
}

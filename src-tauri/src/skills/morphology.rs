// Morphology Skill - 词根拆解查询

use super::{Skill, SkillInput, SkillOutput};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 词根拆解结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphologyEntry {
    pub word: String,
    /// 拆解结果（如 "archi.tect.ure"）
    pub segmentation: String,
    /// 词性
    pub pos: Option<String>,
    /// 拆分的词根列表
    pub parts: Vec<MorphologyPart>,
}

/// 词根部分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphologyPart {
    /// 词根（如 "archi-"）
    pub part: String,
    /// 类型（prefix/root/suffix）
    pub part_type: String,
    /// 含义（需要从其他来源获取）
    pub meaning: Option<String>,
}

/// Morphology Skill - 词根拆解
pub struct MorphologySkill {
    /// 内存中的词根数据（word -> segmentation）
    data: Arc<HashMap<String, (String, Option<String>)>>,
}

impl MorphologySkill {
    /// 创建 MorphologySkill（从 MorphoLex 数据加载）
    pub fn new(data: HashMap<String, (String, Option<String>)>) -> Self {
        Self {
            data: Arc::new(data),
        }
    }

    /// 从 CSV 文件加载（MorphoLex 格式）
    pub async fn from_csv(path: &str) -> Result<Self> {
        use tokio::fs::File;
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut data = HashMap::new();
        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // 跳过表头
        if let Some(_header) = lines.next_line().await? {
            // Header: Word,MorphoLexSegm,MorphoLexPOS
        }

        // 读取数据
        while let Some(line) = lines.next_line().await? {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                let word = parts[0].trim().to_lowercase();
                let segmentation = parts[1].trim().to_string();
                let pos = if parts.len() >= 3 {
                    Some(parts[2].trim().to_string())
                } else {
                    None
                };

                data.insert(word, (segmentation, pos));
            }
        }

        Ok(Self::new(data))
    }

    /// 查询单词的词根拆解
    fn lookup_morphology(&self, word: &str) -> Option<MorphologyEntry> {
        let word_lower = word.to_lowercase();
        self.data.get(&word_lower).map(|(segmentation, pos)| {
            let parts = self.parse_segmentation(segmentation);

            MorphologyEntry {
                word: word.to_string(),
                segmentation: segmentation.clone(),
                pos: pos.clone(),
                parts,
            }
        })
    }

    /// 解析拆分字符串（如 "archi.tect.ure" -> ["archi", "tect", "ure"]）
    fn parse_segmentation(&self, segmentation: &str) -> Vec<MorphologyPart> {
        segmentation
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

                MorphologyPart {
                    part: part.to_string(),
                    part_type: part_type.to_string(),
                    meaning: None, // 需要从其他数据源获取
                }
            })
            .collect()
    }
}

#[async_trait]
impl Skill for MorphologySkill {
    fn name(&self) -> &str {
        "morphology"
    }

    fn description(&self) -> &str {
        "查询单词的词根拆解（基于 MorphoLex 数据）"
    }

    async fn execute(&self, input: SkillInput) -> Result<SkillOutput> {
        let word = input.primary.to_lowercase();

        if let Some(entry) = self.lookup_morphology(&word) {
            Ok(SkillOutput::from_json(&entry)?
                .with_metadata("source", serde_json::json!("morpholex"))
                .with_metadata("found", serde_json::json!(true)))
        } else {
            Ok(SkillOutput::new(serde_json::json!(null))
                .with_metadata("source", serde_json::json!("morpholex"))
                .with_metadata("found", serde_json::json!(false)))
        }
    }

    fn validate_input(&self, input: &SkillInput) -> Result<()> {
        if input.primary.is_empty() {
            anyhow::bail!("单词不能为空");
        }
        Ok(())
    }

    fn is_available(&self) -> bool {
        !self.data.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_segmentation() {
        let skill = MorphologySkill::new(HashMap::new());

        let parts = skill.parse_segmentation("archi.tect.ure");
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].part, "archi");
        assert_eq!(parts[1].part, "tect");
        assert_eq!(parts[2].part, "ure");
    }

    #[tokio::test]
    async fn test_morphology_lookup() {
        let mut data = HashMap::new();
        data.insert(
            "brilliant".to_string(),
            ("brill.i.ant".to_string(), Some("ADJ".to_string())),
        );

        let skill = MorphologySkill::new(data);

        let input = SkillInput::new("brilliant");
        let output = skill.execute(input).await.unwrap();

        assert_eq!(output.metadata["found"], true);

        let entry: MorphologyEntry = output.into_type().unwrap();
        assert_eq!(entry.word, "brilliant");
        assert_eq!(entry.segmentation, "brill.i.ant");
        assert_eq!(entry.parts.len(), 3);
    }

    #[tokio::test]
    async fn test_morphology_not_found() {
        let skill = MorphologySkill::new(HashMap::new());

        let input = SkillInput::new("unknown");
        let output = skill.execute(input).await.unwrap();

        assert_eq!(output.metadata["found"], false);
    }
}

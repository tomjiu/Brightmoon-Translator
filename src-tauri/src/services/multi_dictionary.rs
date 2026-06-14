// Multi-source Dictionary Service
// 多源词典聚合服务

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub word: String,
    pub phonetics: Vec<Phonetic>,
    pub meanings: Vec<Meaning>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phonetic {
    pub text: Option<String>,
    pub audio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meaning {
    pub part_of_speech: String,
    pub definitions: Vec<Definition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    pub definition: String,
    pub example: Option<String>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
}

// dictionaryapi.dev Response
#[derive(Debug, Deserialize)]
struct DictionaryApiResponse {
    word: String,
    phonetics: Vec<DictionaryApiPhonetic>,
    meanings: Vec<DictionaryApiMeaning>,
}

#[derive(Debug, Deserialize)]
struct DictionaryApiPhonetic {
    text: Option<String>,
    audio: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DictionaryApiMeaning {
    #[serde(rename = "partOfSpeech")]
    part_of_speech: String,
    definitions: Vec<DictionaryApiDefinition>,
}

#[derive(Debug, Deserialize)]
struct DictionaryApiDefinition {
    definition: String,
    example: Option<String>,
    #[serde(default)]
    synonyms: Vec<String>,
    #[serde(default)]
    antonyms: Vec<String>,
}

pub struct MultiSourceDictionary {
    client: Client,
}

impl MultiSourceDictionary {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();

        Self { client }
    }

    /// 查询单词（多源聚合）
    pub async fn lookup(&self, word: &str) -> anyhow::Result<Vec<DictionaryEntry>> {
        let mut results = Vec::new();

        // 1. 优先使用 dictionaryapi.dev（最推荐）
        if let Ok(entry) = self.lookup_dictionaryapi(word).await {
            results.push(entry);
        }

        // 2. 如果没有结果，尝试 FreeDictionaryAPI
        if results.is_empty() {
            if let Ok(entry) = self.lookup_free_dictionary(word).await {
                results.push(entry);
            }
        }

        // 3. 最后尝试本地 ECDICT（作为兜底）
        // （这个在另一个 command 中实现）

        if results.is_empty() {
            anyhow::bail!("No results found for '{}'", word);
        }

        Ok(results)
    }

    /// dictionaryapi.dev - 最推荐的免费 API
    async fn lookup_dictionaryapi(&self, word: &str) -> anyhow::Result<DictionaryEntry> {
        let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", word);

        let response = self.client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("API request failed: {}", response.status());
        }

        let data: Vec<DictionaryApiResponse> = response.json().await?;

        if data.is_empty() {
            anyhow::bail!("No results");
        }

        let first = &data[0];

        Ok(DictionaryEntry {
            word: first.word.clone(),
            phonetics: first.phonetics.iter().map(|p| Phonetic {
                text: p.text.clone(),
                audio: p.audio.clone(),
            }).collect(),
            meanings: first.meanings.iter().map(|m| Meaning {
                part_of_speech: m.part_of_speech.clone(),
                definitions: m.definitions.iter().map(|d| Definition {
                    definition: d.definition.clone(),
                    example: d.example.clone(),
                    synonyms: d.synonyms.clone(),
                    antonyms: d.antonyms.clone(),
                }).collect(),
            }).collect(),
            source: "DictionaryAPI.dev".to_string(),
        })
    }

    /// FreeDictionaryAPI (Wiktionary-based)
    async fn lookup_free_dictionary(&self, word: &str) -> anyhow::Result<DictionaryEntry> {
        // 使用相同的 API endpoint，它们兼容
        let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", word);

        let response = self.client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("API request failed");
        }

        let data: Vec<DictionaryApiResponse> = response.json().await?;

        if data.is_empty() {
            anyhow::bail!("No results");
        }

        let first = &data[0];

        Ok(DictionaryEntry {
            word: first.word.clone(),
            phonetics: first.phonetics.iter().map(|p| Phonetic {
                text: p.text.clone(),
                audio: p.audio.clone(),
            }).collect(),
            meanings: first.meanings.iter().map(|m| Meaning {
                part_of_speech: m.part_of_speech.clone(),
                definitions: m.definitions.iter().map(|d| Definition {
                    definition: d.definition.clone(),
                    example: d.example.clone(),
                    synonyms: d.synonyms.clone(),
                    antonyms: d.antonyms.clone(),
                }).collect(),
            }).collect(),
            source: "FreeDictionaryAPI".to_string(),
        })
    }

    /// 有道词典（网页端免费接口）
    pub async fn lookup_youdao(&self, word: &str) -> anyhow::Result<String> {
        // 有道网页版查词接口
        let url = format!(
            "https://dict.youdao.com/jsonapi?q={}&le=en&dicts={{\"count\":99}}",
            urlencoding::encode(word)
        );

        let response = self.client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?;

        let text = response.text().await?;
        Ok(text)
    }

    /// Iciba（金山词霸）免费接口
    pub async fn lookup_iciba(&self, word: &str) -> anyhow::Result<String> {
        let url = format!("http://dict-co.iciba.com/api/dictionary.php?w={}&type=json", word);

        let response = self.client
            .get(&url)
            .send()
            .await?;

        let text = response.text().await?;
        Ok(text)
    }
}

impl Default for MultiSourceDictionary {
    fn default() -> Self {
        Self::new()
    }
}

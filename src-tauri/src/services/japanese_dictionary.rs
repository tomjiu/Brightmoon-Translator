// Japanese Dictionary Service — Jisho.org API + JMdict scaffolding
//
// Jisho.org provides free access to JMdict data via a REST API.
// JMdict (Japanese-Multilingual Dictionary) is the de facto standard
// open-source Japanese-English dictionary with ~200,000 entries.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A Japanese word entry with readings, meanings, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JapaneseEntry {
    pub word: String,
    pub reading: String,
    pub meanings: Vec<JapaneseMeaning>,
    pub jlpt_level: Option<String>,
    pub is_common: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JapaneseMeaning {
    pub english_definitions: Vec<String>,
    pub parts_of_speech: Vec<String>,
    pub tags: Vec<String>,
}

// Jisho.org API response types
#[derive(Debug, Deserialize)]
struct JishoResponse {
    data: Vec<JishoEntry>,
}

#[derive(Debug, Deserialize)]
struct JishoEntry {
    slug: String,
    japanese: Vec<JishoJapanese>,
    senses: Vec<JishoSense>,
    jlpt: Vec<String>,
    is_common: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct JishoJapanese {
    #[allow(dead_code)]
    word: Option<String>,
    reading: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JishoSense {
    english_definitions: Vec<String>,
    parts_of_speech: Vec<String>,
    tags: Vec<String>,
}

pub struct JapaneseDictionary {
    client: Client,
}

impl JapaneseDictionary {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();

        Self { client }
    }

    /// Look up a Japanese word using the Jisho.org API.
    pub async fn lookup(&self, word: &str) -> anyhow::Result<Vec<JapaneseEntry>> {
        let url = format!("https://jisho.org/api/v1/search/words?keyword={}", urlencoding::encode(word));

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "MoonTranslator/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Jisho API request failed: {}", response.status());
        }

        let data: JishoResponse = response.json().await?;

        if data.data.is_empty() {
            anyhow::bail!("No results found for '{word}'");
        }

        let entries: Vec<JapaneseEntry> = data
            .data
            .iter()
            .take(5) // Limit to top 5 matches
            .map(|entry| {
                let reading = entry
                    .japanese
                    .first()
                    .and_then(|j| j.reading.clone())
                    .unwrap_or_default();

                let jlpt_level = entry
                    .jlpt
                    .iter()
                    .filter_map(|s| s.strip_prefix("jlpt-n"))
                    .map(|s| format!("N{s}"))
                    .next();

                JapaneseEntry {
                    word: entry.slug.clone(),
                    reading,
                    meanings: entry
                        .senses
                        .iter()
                        .take(10) // Limit meanings per entry
                        .map(|s| JapaneseMeaning {
                            english_definitions: s.english_definitions.clone(),
                            parts_of_speech: s.parts_of_speech.clone(),
                            tags: s.tags.clone(),
                        })
                        .collect(),
                    jlpt_level,
                    is_common: entry.is_common.unwrap_or(false),
                    source: "Jisho.org (JMdict)".to_string(),
                }
            })
            .collect();

        Ok(entries)
    }
}

impl Default for JapaneseDictionary {
    fn default() -> Self {
        Self::new()
    }
}
use reqwest::Client;
use serde::Deserialize;

// Re-export shared types from models
pub use crate::models::dictionary::{Definition, DictionaryResult, Meaning};

// Youdao API response structures
#[derive(Deserialize)]
struct YoudaoResponse {
    #[serde(default)]
    ec: Option<EcDict>,
    #[serde(default)]
    ce: Option<CeDict>,
}

#[derive(Deserialize)]
struct EcDict {
    #[serde(default)]
    word: Option<EcWord>,
}

#[derive(Deserialize)]
struct EcWord {
    #[serde(default)]
    trs: Vec<EcTr>,
    #[serde(default)]
    ukphone: Option<String>,
    #[serde(default)]
    usphone: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    ukspeech: Option<String>,
    #[serde(default)]
    usspeech: Option<String>,
}

#[derive(Deserialize)]
struct EcTr {
    #[serde(default)]
    tr: Vec<EcTrItem>,
}

#[derive(Deserialize)]
struct EcTrItem {
    #[serde(default)]
    l: Option<EcL>,
}

#[derive(Deserialize)]
struct EcL {
    #[serde(default)]
    i: Vec<String>,
}

#[derive(Deserialize)]
struct CeDict {
    #[serde(default)]
    word: Option<CeWord>,
}

#[derive(Deserialize)]
struct CeWord {
    #[serde(default)]
    trs: Vec<CeTr>,
    #[serde(default)]
    phone: Option<String>,
}

#[derive(Deserialize)]
struct CeTr {
    #[serde(default)]
    tr: Vec<CeTrItem>,
}

#[derive(Deserialize)]
struct CeTrItem {
    #[serde(default)]
    l: Option<CeL>,
}

#[derive(Deserialize)]
struct CeL {
    #[serde(default)]
    i: Vec<String>,
}

pub struct Dictionary {
    client: Client,
}

impl Dictionary {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Lookup English word using Youdao Dictionary API
    pub async fn lookup(&self, word: &str) -> anyhow::Result<Vec<DictionaryResult>> {
        let q = format!("bk:{}", word);
        let dicts = r#"{"count":2,"dicts":[["ec","ce"],["web_trans"]]}"#;
        let url = format!(
            "https://dict.youdao.com/jsonapi_s?q={}&le=en&dicts={}",
            urlencoding::encode(&q),
            urlencoding::encode(dicts)
        );

        let resp = self.client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Youdao API error: {}", resp.status()));
        }

        let body: YoudaoResponse = resp.json().await?;

        // Try to parse EC (English-Chinese) dictionary
        if let Some(ec) = body.ec {
            if let Some(word_data) = ec.word {
                let phonetic = if let Some(uk) = &word_data.ukphone {
                    if let Some(us) = &word_data.usphone {
                        Some(format!("UK: {} / US: {}", uk, us))
                    } else {
                        Some(format!("UK: {}", uk))
                    }
                } else {
                    word_data.phone.map(|p| format!("/{}", p))
                };

                let meanings: Vec<Meaning> = word_data
                    .trs
                    .into_iter()
                    .enumerate()
                    .map(|(i, tr)| {
                        let definitions: Vec<Definition> = tr
                            .tr
                            .into_iter()
                            .filter_map(|tr_item| {
                                tr_item.l.and_then(|l| {
                                    l.i.first().map(|def| Definition {
                                        definition: def.clone(),
                                        example: None,
                                        synonyms: vec![],
                                        antonyms: vec![],
                                    })
                                })
                            })
                            .collect();

                        Meaning {
                            part_of_speech: if i == 0 { "基本释义".to_string() } else { "扩展释义".to_string() },
                            definitions,
                        }
                    })
                    .filter(|m| !m.definitions.is_empty())
                    .collect();

                if !meanings.is_empty() {
                    return Ok(vec![DictionaryResult {
                        word: word.to_string(),
                        phonetic,
                        meanings,
                        source_urls: vec![],
                    }]);
                }
            }
        }

        // Fallback: return a basic result
        Ok(vec![DictionaryResult {
            word: word.to_string(),
            phonetic: None,
            meanings: vec![Meaning {
                part_of_speech: "未找到释义".to_string(),
                definitions: vec![Definition {
                    definition: format!("未找到单词 \"{}\" 的释义", word),
                    example: None,
                    synonyms: vec![],
                    antonyms: vec![],
                }],
            }],
            source_urls: vec![],
        }])
    }

    /// Lookup Chinese word using Youdao Dictionary API
    pub async fn lookup_chinese(&self, word: &str) -> anyhow::Result<Vec<DictionaryResult>> {
        let q = format!("bk:{}", word);
        let dicts = r#"{"count":2,"dicts":[["ce","ec"],["web_trans"]]}"#;
        let url = format!(
            "https://dict.youdao.com/jsonapi_s?q={}&le=zh&dicts={}",
            urlencoding::encode(&q),
            urlencoding::encode(dicts)
        );

        let resp = self.client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Youdao API error: {}", resp.status()));
        }

        let body: YoudaoResponse = resp.json().await?;

        // Try to parse CE (Chinese-English) dictionary
        if let Some(ce) = body.ce {
            if let Some(word_data) = ce.word {
                let phonetic = word_data.phone.map(|p| format!("[{}]", p));

                let meanings: Vec<Meaning> = word_data
                    .trs
                    .into_iter()
                    .enumerate()
                    .map(|(i, tr)| {
                        let definitions: Vec<Definition> = tr
                            .tr
                            .into_iter()
                            .filter_map(|tr_item| {
                                tr_item.l.and_then(|l| {
                                    l.i.first().map(|def| Definition {
                                        definition: def.clone(),
                                        example: None,
                                        synonyms: vec![],
                                        antonyms: vec![],
                                    })
                                })
                            })
                            .collect();

                        Meaning {
                            part_of_speech: if i == 0 { "基本释义".to_string() } else { "扩展释义".to_string() },
                            definitions,
                        }
                    })
                    .filter(|m| !m.definitions.is_empty())
                    .collect();

                if !meanings.is_empty() {
                    return Ok(vec![DictionaryResult {
                        word: word.to_string(),
                        phonetic,
                        meanings,
                        source_urls: vec![],
                    }]);
                }
            }
        }

        // Fallback: return a basic result
        Ok(vec![DictionaryResult {
            word: word.to_string(),
            phonetic: None,
            meanings: vec![Meaning {
                part_of_speech: "未找到释义".to_string(),
                definitions: vec![Definition {
                    definition: format!("未找到词语 \"{}\" 的释义", word),
                    example: None,
                    synonyms: vec![],
                    antonyms: vec![],
                }],
            }],
            source_urls: vec![],
        }])
    }
}

/// Check if text is a single word (for dictionary lookup)
/// Supports both English words and CJK characters
pub fn is_single_word(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 50 {
        return false;
    }

    // Check if contains CJK characters
    let has_cjk = trimmed.chars().any(|c| {
        matches!(c,
            '\u{4e00}'..='\u{9fff}' |  // CJK Unified Ideographs
            '\u{3400}'..='\u{4dbf}' |  // CJK Unified Ideographs Extension A
            '\u{f900}'..='\u{faff}'    // CJK Compatibility Ideographs
        )
    });

    if has_cjk {
        // For CJK text: allow up to 10 characters (Chinese phrases)
        let cjk_count = trimmed.chars().filter(|c| {
            matches!(c,
                '\u{4e00}'..='\u{9fff}' |
                '\u{3400}'..='\u{4dbf}' |
                '\u{f900}'..='\u{faff}'
            )
        }).count();
        return cjk_count >= 1 && cjk_count <= 10;
    }

    // For English: no spaces
    !trimmed.contains(char::is_whitespace)
}

/// Detect if text contains CJK characters
pub fn is_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{4e00}'..='\u{9fff}' |
            '\u{3400}'..='\u{4dbf}' |
            '\u{f900}'..='\u{faff}'
        )
    })
}

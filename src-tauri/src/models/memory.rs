use serde::{Deserialize, Serialize};

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

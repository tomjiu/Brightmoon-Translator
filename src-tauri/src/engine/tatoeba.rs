//! Tatoeba example sentences — pot-app-translate-plugin-tatoeba style.

use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;

pub struct TatoebaEngine {
    client: Client,
}

impl Default for TatoebaEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TatoebaEngine {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }
}

fn map_lang(code: &str) -> String {
    let c = code.trim().to_ascii_lowercase();
    match c.as_str() {
        "auto" | "" => "und".into(),
        "zh" | "zh-cn" | "zh-hans" => "cmn".into(),
        "zh-tw" | "zh-hant" => "cmn".into(),
        "en" | "eng" => "eng".into(),
        "ja" | "jp" => "jpn".into(),
        "ko" => "kor".into(),
        "fr" => "fra".into(),
        "de" => "deu".into(),
        "es" => "spa".into(),
        "ru" => "rus".into(),
        "pt" => "por".into(),
        "it" => "ita".into(),
        other if other.len() == 3 => other.to_string(),
        other => other.chars().take(3).collect(),
    }
}

fn format_results(body: &serde_json::Value, limit: usize) -> String {
    let mut lines = Vec::new();
    let Some(results) = body.get("results").and_then(|r| r.as_array()) else {
        return String::new();
    };
    for item in results.iter().take(limit) {
        let source = item
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim();
        if source.is_empty() {
            continue;
        }
        let mut targets = Vec::new();
        // translations may be array of arrays or array of objects
        if let Some(trans) = item.get("translations") {
            collect_translation_texts(trans, &mut targets);
        }
        if targets.is_empty() {
            lines.push(source.to_string());
        } else {
            let tgt = targets.join(" / ");
            lines.push(format!("{source} ⇒ {tgt}"));
        }
    }
    lines.join("\n")
}

fn collect_translation_texts(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_translation_texts(item, out);
            }
        },
        serde_json::Value::Object(map) => {
            if let Some(t) = map.get("text").and_then(|t| t.as_str()) {
                let t = t.trim();
                if !t.is_empty() && !out.iter().any(|x| x == t) {
                    out.push(t.to_string());
                }
            }
            for (_k, val) in map {
                if val.is_array() || val.is_object() {
                    // only dive into nested translation groups, avoid huge trees
                    if map.contains_key("text") {
                        continue;
                    }
                    collect_translation_texts(val, out);
                }
            }
        },
        _ => {},
    }
}

#[async_trait]
impl TranslationEngine for TatoebaEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let q = text.trim();
        if q.is_empty() {
            anyhow::bail!("Empty query");
        }
        let from_l = map_lang(from);
        let to_l = map_lang(to);
        let url = "https://tatoeba.org/eng/api_v0/search";
        let resp = self
            .client
            .get(url)
            .query(&[
                ("query", q),
                ("from", from_l.as_str()),
                ("to", to_l.as_str()),
                ("has_audio", "no"),
                ("sort", "relevance"),
            ])
            .header("User-Agent", "MoonTranslator/1.0")
            .send()
            .await?;
        super::check_response(&resp, "Tatoeba")?;
        let body: serde_json::Value = resp.json().await?;
        let formatted = format_results(&body, 5);
        if formatted.is_empty() {
            anyhow::bail!("Tatoeba: no example sentences found");
        }
        Ok(formatted)
    }

    fn name(&self) -> &'static str {
        "Tatoeba"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_zh_en() {
        assert_eq!(map_lang("zh"), "cmn");
        assert_eq!(map_lang("en"), "eng");
    }

    #[test]
    fn format_pair() {
        let body = serde_json::json!({
            "results": [{
                "text": "Hello",
                "translations": [[{ "text": "你好" }]]
            }]
        });
        let s = format_results(&body, 5);
        assert!(s.contains("Hello"));
        assert!(s.contains("你好"));
    }
}

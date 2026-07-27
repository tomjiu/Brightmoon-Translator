//! Eudic / 欧陆词典 open API (api.frdic.com) — aligned with pot + STranslate.

use super::{CollectionItem, CollectionTargetResult};
use crate::models::config::EudicCollectionConfig;

const BASE: &str = "https://api.frdic.com/api/open/v1";

fn truncate_body(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 200 {
        format!("{}…", t.chars().take(200).collect::<String>())
    } else {
        t.to_string()
    }
}

pub async fn push(
    client: &reqwest::Client,
    cfg: &EudicCollectionConfig,
    item: &CollectionItem,
) -> CollectionTargetResult {
    let token = cfg.token.trim();
    if token.is_empty() {
        return CollectionTargetResult {
            target: "eudic".into(),
            ok: false,
            message: "Eudic token is empty".into(),
        };
    }
    let word = item.word.trim();
    if word.is_empty() {
        return CollectionTargetResult {
            target: "eudic".into(),
            ok: false,
            message: "Word is empty".into(),
        };
    }

    match push_inner(client, token, &cfg.book_name, item).await {
        Ok(msg) => CollectionTargetResult {
            target: "eudic".into(),
            ok: true,
            message: msg,
        },
        Err(e) => CollectionTargetResult {
            target: "eudic".into(),
            ok: false,
            message: e,
        },
    }
}

async fn push_inner(
    client: &reqwest::Client,
    token: &str,
    book_name: &str,
    item: &CollectionItem,
) -> Result<String, String> {
    let category_id = resolve_category(client, token, book_name).await?;
    let word = item.word.trim();

    if !item.note.trim().is_empty() {
        // STranslate path: single word + note
        add_word_with_categories(client, token, word, &category_id).await?;
        match add_note(client, token, word, item.note.trim()).await {
            Ok(()) => Ok(format!("Added to Eudic book with note (id={category_id})")),
            Err(e) => Ok(format!(
                "Added to Eudic book (id={category_id}); note failed: {e}"
            )),
        }
    } else {
        add_words_bulk(client, token, &category_id, word).await?;
        Ok(format!("Added to Eudic book (id={category_id})"))
    }
}

async fn resolve_category(
    client: &reqwest::Client,
    token: &str,
    book_name: &str,
) -> Result<String, String> {
    let name = if book_name.trim().is_empty() {
        "Moon"
    } else {
        book_name.trim()
    };

    let res = client
        .get(format!("{BASE}/studylist/category"))
        .query(&[("language", "en")])
        .header("Authorization", token)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Eudic list category failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Eudic list category HTTP {status}: {}",
            truncate_body(&body)
        ));
    }

    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Eudic category JSON: {e}"))?;

    if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
        for entry in arr {
            if entry.get("name").and_then(|n| n.as_str()) == Some(name) {
                if let Some(id) = entry.get("id").map(|i| match i {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => String::new(),
                }) {
                    if !id.is_empty() {
                        return Ok(id);
                    }
                }
            }
        }
    }

    // Create category
    let res = client
        .post(format!("{BASE}/studylist/category"))
        .header("Authorization", token)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "language": "en",
            "name": name,
        }))
        .send()
        .await
        .map_err(|e| format!("Eudic create category failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Eudic create category HTTP {status}: {}",
            truncate_body(&body)
        ));
    }

    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Eudic create category JSON: {e}"))?;

    let id = v
        .pointer("/data/id")
        .map(|i| match i {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Eudic create category: no id in {}", truncate_body(&body)))?;

    Ok(id)
}

async fn add_words_bulk(
    client: &reqwest::Client,
    token: &str,
    category_id: &str,
    word: &str,
) -> Result<(), String> {
    let res = client
        .post(format!("{BASE}/studylist/words"))
        .header("Authorization", token)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "id": category_id,
            "language": "en",
            "words": [word],
        }))
        .send()
        .await
        .map_err(|e| format!("Eudic add words failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Eudic add words HTTP {status}: {}",
            truncate_body(&body)
        ));
    }
    Ok(())
}

async fn add_word_with_categories(
    client: &reqwest::Client,
    token: &str,
    word: &str,
    category_id: &str,
) -> Result<(), String> {
    let res = client
        .post(format!("{BASE}/studylist/word"))
        .header("Authorization", token)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "language": "en",
            "word": word,
            "category_ids": [category_id],
        }))
        .send()
        .await
        .map_err(|e| format!("Eudic add word failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Eudic add word HTTP {status}: {}",
            truncate_body(&body)
        ));
    }
    Ok(())
}

async fn add_note(
    client: &reqwest::Client,
    token: &str,
    word: &str,
    note: &str,
) -> Result<(), String> {
    let res = client
        .post(format!("{BASE}/studylist/note"))
        .header("Authorization", token)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "language": "en",
            "word": word,
            "note": note,
        }))
        .send()
        .await
        .map_err(|e| format!("Eudic add note failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Eudic add note HTTP {status}: {}",
            truncate_body(&body)
        ));
    }
    Ok(())
}

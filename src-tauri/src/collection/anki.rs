//! AnkiConnect collection — aligned with pot-desktop Anki service.

use super::{CollectionItem, CollectionTargetResult};
use crate::models::config::AnkiCollectionConfig;

pub async fn push(
    client: &reqwest::Client,
    cfg: &AnkiCollectionConfig,
    item: &CollectionItem,
) -> CollectionTargetResult {
    let word = item.word.trim();
    if word.is_empty() {
        return CollectionTargetResult {
            target: "anki".into(),
            ok: false,
            message: "Word is empty".into(),
        };
    }

    match push_inner(client, cfg, item).await {
        Ok(msg) => CollectionTargetResult {
            target: "anki".into(),
            ok: true,
            message: msg,
        },
        Err(e) => CollectionTargetResult {
            target: "anki".into(),
            ok: false,
            message: e,
        },
    }
}

async fn push_inner(
    client: &reqwest::Client,
    cfg: &AnkiCollectionConfig,
    item: &CollectionItem,
) -> Result<String, String> {
    let port = if cfg.port == 0 { 8765 } else { cfg.port };
    let deck = if cfg.deck.trim().is_empty() {
        "Moon"
    } else {
        cfg.deck.trim()
    };
    let model = if cfg.model.trim().is_empty() {
        "Moon Card"
    } else {
        cfg.model.trim()
    };

    let _ = anki_invoke(
        client,
        port,
        "createDeck",
        serde_json::json!({ "deck": deck }),
    )
    .await?;

    // createModel is not fully idempotent; ignore "already exists" style errors.
    if let Err(e) = anki_invoke(
        client,
        port,
        "createModel",
        serde_json::json!({
            "modelName": model,
            "inOrderFields": ["Front", "Back"],
            "isCloze": false,
            "cardTemplates": [{
                "Name": model,
                "Front": "{{Front}}",
                "Back": "{{FrontSide}}<hr id=answer>{{Back}}"
            }]
        }),
    )
    .await
    {
        let lower = e.to_lowercase();
        if !lower.contains("exists") && !lower.contains("duplicate") {
            // continue anyway — model may already exist with different message
            tracing::debug!("Anki createModel: {e}");
        }
    }

    let mut back = item.translation.clone();
    if !item.note.trim().is_empty() {
        if !back.is_empty() {
            back.push_str("<br>");
        }
        back.push_str(item.note.trim());
    }
    if back.is_empty() {
        back = item.word.clone();
    }

    anki_invoke(
        client,
        port,
        "addNote",
        serde_json::json!({
            "note": {
                "deckName": deck,
                "modelName": model,
                "fields": {
                    "Front": item.word.trim(),
                    "Back": back,
                },
                "options": {
                    "allowDuplicate": true
                },
                "tags": ["moontranslator"]
            }
        }),
    )
    .await?;

    Ok(format!("Added note to Anki deck '{deck}'"))
}

async fn anki_invoke(
    client: &reqwest::Client,
    port: u16,
    action: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let url = format!("http://127.0.0.1:{port}");
    let body = serde_json::json!({
        "action": action,
        "version": 6,
        "params": params,
    });
    let res = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("AnkiConnect unreachable on port {port}: {e}"))?;

    let status = res.status();
    let v: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("AnkiConnect invalid JSON ({status}): {e}"))?;

    if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
        if let Some(s) = err.as_str() {
            if !s.is_empty() {
                return Err(format!("AnkiConnect error: {s}"));
            }
        } else {
            return Err(format!("AnkiConnect error: {err}"));
        }
    }
    Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null))
}

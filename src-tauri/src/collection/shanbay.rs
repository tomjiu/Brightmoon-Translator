//! Shanbay / 扇贝生词本 — aligned with pot-app-collection-plugin-shanbay.

use super::{CollectionItem, CollectionTargetResult};
use crate::models::config::ShanbayCollectionConfig;

const UPLOAD_URL: &str = "https://apiv3.shanbay.com/wordscollection/words_bulk_upload";

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
    cfg: &ShanbayCollectionConfig,
    item: &CollectionItem,
) -> CollectionTargetResult {
    let token = cfg.credential.trim();
    if token.is_empty() {
        return CollectionTargetResult {
            target: "shanbay".into(),
            ok: false,
            message: "Shanbay auth_token is empty (paste Cookie auth_token from web login)".into(),
        };
    }
    let word = item.word.trim();
    if word.is_empty() {
        return CollectionTargetResult {
            target: "shanbay".into(),
            ok: false,
            message: "Word is empty".into(),
        };
    }

    match push_inner(client, token, word).await {
        Ok(msg) => CollectionTargetResult {
            target: "shanbay".into(),
            ok: true,
            message: msg,
        },
        Err(e) => CollectionTargetResult {
            target: "shanbay".into(),
            ok: false,
            message: e,
        },
    }
}

async fn push_inner(
    client: &reqwest::Client,
    auth_token: &str,
    word: &str,
) -> Result<String, String> {
    // pot plugin uses business_id: 6 for bulk upload
    let res = client
        .post(UPLOAD_URL)
        .header("Cookie", format!("auth_token={auth_token}"))
        .header("Content-Type", "application/json;charset=UTF-8")
        .json(&serde_json::json!({
            "business_id": 6,
            "words": [word],
        }))
        .send()
        .await
        .map_err(|e| format!("Shanbay upload failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Shanbay upload HTTP {status}: {}",
            truncate_body(&body)
        ));
    }

    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Shanbay JSON: {e} — {}", truncate_body(&body)))?;

    if let Some(msg) = v.get("msg").and_then(|m| m.as_str()) {
        if !msg.is_empty() {
            return Err(format!("Shanbay: {msg}"));
        }
    }

    if let Some(task_id) = v.get("task_id").and_then(|t| {
        t.as_str()
            .map(std::string::ToString::to_string)
            .or_else(|| t.as_i64().map(|n| n.to_string()))
    }) {
        // Poll task status once (pot does a single GET check)
        let check = client
            .get(UPLOAD_URL)
            .query(&[("business_id", "6"), ("task_id", task_id.as_str())])
            .header("Cookie", format!("auth_token={auth_token}"))
            .send()
            .await
            .map_err(|e| format!("Shanbay task check failed: {e}"))?;

        let check_status = check.status();
        let check_body = check.text().await.unwrap_or_default();
        if !check_status.is_success() {
            return Err(format!(
                "Shanbay task check HTTP {check_status}: {}",
                truncate_body(&check_body)
            ));
        }
        if let Ok(cv) = serde_json::from_str::<serde_json::Value>(&check_body) {
            let failed = cv.get("failed_count").and_then(serde_json::Value::as_i64).unwrap_or(0);
            if failed > 0 {
                return Err("Shanbay: failed to add words (failed_count > 0)".into());
            }
        }
        return Ok(format!("Added to Shanbay (task_id={task_id})"));
    }

    // Some responses may succeed without task_id
    Ok(format!("Shanbay accepted: {}", truncate_body(&body)))
}

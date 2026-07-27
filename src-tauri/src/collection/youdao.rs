//! Youdao wordbook — aligned with pot-app-collection-plugin-youdao.

use super::{CollectionItem, CollectionTargetResult};
use crate::models::config::YoudaoCollectionConfig;

const ADD_URL: &str = "https://dict.youdao.com/wordbook/webapi/v2/ajax/add";

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
    cfg: &YoudaoCollectionConfig,
    item: &CollectionItem,
) -> CollectionTargetResult {
    let cookie = cfg.cookie.trim();
    if cookie.is_empty() {
        return CollectionTargetResult {
            target: "youdao".into(),
            ok: false,
            message: "Youdao cookie is empty (paste full Cookie from dict.youdao.com after login)"
                .into(),
        };
    }
    let word = item.word.trim();
    if word.is_empty() {
        return CollectionTargetResult {
            target: "youdao".into(),
            ok: false,
            message: "Word is empty".into(),
        };
    }

    let lan = if cfg.lan.trim().is_empty() {
        "en"
    } else {
        cfg.lan.trim()
    };

    match push_inner(client, cookie, word, lan).await {
        Ok(msg) => CollectionTargetResult {
            target: "youdao".into(),
            ok: true,
            message: msg,
        },
        Err(e) => CollectionTargetResult {
            target: "youdao".into(),
            ok: false,
            message: e,
        },
    }
}

async fn push_inner(
    client: &reqwest::Client,
    cookie: &str,
    word: &str,
    lan: &str,
) -> Result<String, String> {
    let res = client
        .post(ADD_URL)
        .header("Cookie", cookie)
        .query(&[("word", word), ("lan", lan)])
        .send()
        .await
        .map_err(|e| format!("Youdao wordbook request failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Youdao wordbook HTTP {status}: {}",
            truncate_body(&body)
        ));
    }

    parse_youdao_add_response(&body)
}

/// Parse pot-compatible JSON: success when `code == 0`.
fn parse_youdao_add_response(body: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| format!("Youdao JSON: {e} — {}", truncate_body(body)))?;

    let code = v
        .get("code")
        .and_then(|c| c.as_i64().or_else(|| c.as_u64().map(|u| u as i64)));

    if code == Some(0) {
        return Ok("Added to Youdao wordbook".into());
    }

    if let Some(msg) = v.get("msg").and_then(|m| m.as_str()) {
        if !msg.is_empty() {
            return Err(format!("Youdao: {msg}"));
        }
    }

    Err(format!(
        "Youdao wordbook failed (code={:?}): {}",
        code,
        truncate_body(body)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_success_code_zero() {
        let r = parse_youdao_add_response(r#"{"code":0,"msg":"ok"}"#).unwrap();
        assert!(r.contains("Added"));
    }

    #[test]
    fn parse_error_msg() {
        let err = parse_youdao_add_response(r#"{"code":1,"msg":"login required"}"#).unwrap_err();
        assert!(err.contains("login required"));
    }
}

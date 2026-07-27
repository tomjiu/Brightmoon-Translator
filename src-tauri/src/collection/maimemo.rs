//! Maimemo / 墨墨背单词 open API — aligned with Bob plugin + open.maimemo.com.

use super::{CollectionItem, CollectionTargetResult};
use crate::models::config::MaimemoCollectionConfig;
use chrono::Local;

const API_BASE: &str = "https://open.maimemo.com/open/api/v1";

fn truncate_body(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 200 {
        format!("{}…", t.chars().take(200).collect::<String>())
    } else {
        t.to_string()
    }
}

fn bearer(token: &str) -> String {
    let t = token.trim();
    if t.to_lowercase().starts_with("bearer ") {
        t.to_string()
    } else {
        format!("Bearer {t}")
    }
}

fn today_date() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Merge words under `# YYYY-MM-DD` heading; skip duplicates (case-insensitive).
pub fn merge_notepad_content(
    content: &str,
    words: &[&str],
    date: &str,
) -> (String, Vec<String>, Vec<String>) {
    let mut lines: Vec<String> = content.lines().map(|l| l.trim_end().to_string()).collect();
    let header = format!("# {date}");

    let mut target_idx = lines.iter().position(|l| l.trim() == header);
    if target_idx.is_none() {
        if !lines.is_empty() && !lines[0].is_empty() {
            lines.insert(0, String::new());
        }
        lines.insert(0, header);
        target_idx = Some(0);
    }
    let insert_at = target_idx.unwrap() + 1;

    let existing: std::collections::HashSet<String> = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .map(|l| l.trim().to_lowercase())
        .collect();

    let mut unique = Vec::new();
    let mut dupes = Vec::new();
    for w in words {
        let trimmed = w.trim();
        if trimmed.is_empty() {
            continue;
        }
        if existing.contains(&trimmed.to_lowercase())
            || unique
                .iter()
                .any(|u: &String| u.eq_ignore_ascii_case(trimmed))
        {
            dupes.push(trimmed.to_string());
        } else {
            unique.push(trimmed.to_string());
        }
    }

    for (i, w) in unique.iter().enumerate() {
        lines.insert(insert_at + i, w.clone());
    }

    (lines.join("\n"), unique, dupes)
}

pub async fn push(
    client: &reqwest::Client,
    cfg: &MaimemoCollectionConfig,
    item: &CollectionItem,
) -> CollectionTargetResult {
    let token = cfg.token.trim();
    if token.is_empty() {
        return CollectionTargetResult {
            target: "maimemo".into(),
            ok: false,
            message: "Maimemo token is empty (App → 我的 → 更多设置 → 实验功能 → 开放API)".into(),
        };
    }
    let word = item.word.trim();
    if word.is_empty() {
        return CollectionTargetResult {
            target: "maimemo".into(),
            ok: false,
            message: "Word is empty".into(),
        };
    }

    match push_inner(client, token, cfg, word).await {
        Ok(msg) => CollectionTargetResult {
            target: "maimemo".into(),
            ok: true,
            message: msg,
        },
        Err(e) => CollectionTargetResult {
            target: "maimemo".into(),
            ok: false,
            message: e,
        },
    }
}

async fn push_inner(
    client: &reqwest::Client,
    token: &str,
    cfg: &MaimemoCollectionConfig,
    word: &str,
) -> Result<String, String> {
    let auth = bearer(token);
    let title = if cfg.notepad_title.trim().is_empty() {
        "Moon"
    } else {
        cfg.notepad_title.trim()
    };
    let date = today_date();

    if cfg.notepad_id.trim().is_empty() {
        return create_notepad(client, &auth, title, word, &date).await;
    }

    add_to_existing(client, &auth, cfg.notepad_id.trim(), word, &date).await
}

async fn create_notepad(
    client: &reqwest::Client,
    auth: &str,
    title: &str,
    word: &str,
    date: &str,
) -> Result<String, String> {
    let content = format!("# {date}\n{word}\n");
    let res = client
        .post(format!("{API_BASE}/notepads"))
        .header("Authorization", auth)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "notepad": {
                "status": "PUBLISHED",
                "content": content,
                "title": title,
                "brief": "Moon translator",
                "tags": ["词典"]
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Maimemo create notepad failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Maimemo create HTTP {status}: {}",
            truncate_body(&body)
        ));
    }

    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Maimemo JSON: {e} — {}", truncate_body(&body)))?;

    if v.get("success").and_then(|s| s.as_bool()) == Some(false) {
        return Err(format!("Maimemo create failed: {}", truncate_body(&body)));
    }

    let id = v
        .pointer("/data/notepad/id")
        .and_then(|x| x.as_str())
        .unwrap_or("");

    if id.is_empty() {
        return Ok(format!(
            "Created notepad (parse id from response): {}",
            truncate_body(&body)
        ));
    }

    Ok(format!(
        "Created notepad and added word; save notepadId={id}"
    ))
}

async fn add_to_existing(
    client: &reqwest::Client,
    auth: &str,
    notepad_id: &str,
    word: &str,
    date: &str,
) -> Result<String, String> {
    let get_url = format!("{API_BASE}/notepads/{notepad_id}");
    let res = client
        .get(&get_url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("Maimemo get notepad failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Maimemo get HTTP {status}: {}",
            truncate_body(&body)
        ));
    }

    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Maimemo JSON: {e} — {}", truncate_body(&body)))?;

    let notepad = v
        .pointer("/data/notepad")
        .cloned()
        .ok_or_else(|| format!("Maimemo: notepad not found — {}", truncate_body(&body)))?;

    let content = notepad
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let np_status = notepad
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("PUBLISHED");
    let title = notepad
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("Moon");
    let brief = notepad
        .get("brief")
        .and_then(|b| b.as_str())
        .unwrap_or("Moon translator");
    let tags = notepad
        .get("tags")
        .cloned()
        .unwrap_or(serde_json::json!(["词典"]));

    let (new_content, unique, dupes) = merge_notepad_content(content, &[word], date);

    if unique.is_empty() && !dupes.is_empty() {
        return Ok(format!("Already in Maimemo notepad: {}", dupes.join(", ")));
    }

    let post = client
        .post(&get_url)
        .header("Authorization", auth)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "notepad": {
                "status": np_status,
                "content": new_content,
                "title": title,
                "brief": brief,
                "tags": tags
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Maimemo update notepad failed: {e}"))?;

    let post_status = post.status();
    let post_body = post.text().await.unwrap_or_default();
    if !post_status.is_success() {
        return Err(format!(
            "Maimemo update HTTP {post_status}: {}",
            truncate_body(&post_body)
        ));
    }

    let pv: serde_json::Value = serde_json::from_str(&post_body).unwrap_or(serde_json::json!({}));
    if pv.get("success").and_then(|s| s.as_bool()) == Some(false) {
        return Err(format!(
            "Maimemo update failed: {}",
            truncate_body(&post_body)
        ));
    }

    let mut parts = Vec::new();
    if !unique.is_empty() {
        parts.push(format!("Added {}", unique.join(", ")));
    }
    if !dupes.is_empty() {
        parts.push(format!("already had {}", dupes.join(", ")));
    }
    Ok(parts.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_inserts_under_today() {
        let (out, unique, dupes) =
            merge_notepad_content("# 2026-01-01\nold\n", &["hello"], "2026-07-27");
        assert!(out.contains("# 2026-07-27"));
        assert!(out.contains("hello"));
        assert_eq!(unique, vec!["hello".to_string()]);
        assert!(dupes.is_empty());
    }

    #[test]
    fn merge_dedupes() {
        let content = "# 2026-07-27\nHello\n";
        let (_out, unique, dupes) =
            merge_notepad_content(content, &["hello", "world"], "2026-07-27");
        assert_eq!(unique, vec!["world".to_string()]);
        assert_eq!(dupes, vec!["hello".to_string()]);
    }
}

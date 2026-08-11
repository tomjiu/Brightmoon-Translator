// LLM Provider Management Commands - 多提供商管理 + 模型列表拉取（openai / anthropic / gemini）

use serde::{Deserialize, Serialize};

/// 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub owned_by: Option<String>,
}

fn normalize_format(api_format: Option<&str>) -> String {
    crate::models::config::normalize_api_format(api_format.unwrap_or("openai"))
}

const ANTHROPIC_FALLBACK: &[&str] = &[
    "claude-sonnet-4-0",
    "claude-opus-4-0",
    "claude-3-5-haiku-latest",
    "claude-3-5-sonnet-latest",
    "claude-3-opus-latest",
];

const GEMINI_FALLBACK: &[&str] = &[
    "gemini-2.0-flash",
    "gemini-2.0-flash-lite",
    "gemini-1.5-pro",
    "gemini-1.5-flash",
];

/// 从 API 端点拉取可用模型列表
#[tauri::command]
pub async fn fetch_available_models(
    base_url: String,
    api_key: String,
    api_format: Option<String>,
) -> Result<Vec<ModelInfo>, String> {
    let format = normalize_format(api_format.as_deref());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    match format.as_str() {
        "anthropic" => fetch_anthropic_models(&client, &base_url, &api_key).await,
        "gemini" => fetch_gemini_models(&client, &base_url, &api_key).await,
        _ => fetch_openai_models(&client, &base_url, &api_key).await,
    }
}

async fn fetch_openai_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<ModelInfo>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {status}: {body}"));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {e}"))?;

    let mut models = Vec::new();

    if let Some(data) = body["data"].as_array() {
        for item in data {
            if let Some(id) = item["id"].as_str() {
                models.push(ModelInfo {
                    id: id.to_string(),
                    name: item["name"].as_str().unwrap_or(id).to_string(),
                    owned_by: item["owned_by"].as_str().map(std::string::ToString::to_string),
                });
            }
        }
    } else if let Some(data) = body["models"].as_array() {
        for item in data {
            if let Some(name) = item["name"].as_str() {
                models.push(ModelInfo {
                    id: name.to_string(),
                    name: name.to_string(),
                    owned_by: item["details"]["family"].as_str().map(std::string::ToString::to_string),
                });
            }
        }
    }

    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

async fn fetch_anthropic_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<ModelInfo>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let response = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let mut models = Vec::new();
            if let Some(data) = body["data"].as_array() {
                for item in data {
                    if let Some(id) = item["id"].as_str() {
                        models.push(ModelInfo {
                            id: id.to_string(),
                            name: item["display_name"].as_str().unwrap_or(id).to_string(),
                            owned_by: Some("anthropic".into()),
                        });
                    }
                }
            }
            if !models.is_empty() {
                models.sort_by(|a, b| a.id.cmp(&b.id));
                return Ok(models);
            }
        },
        _ => {},
    }

    Ok(ANTHROPIC_FALLBACK
        .iter()
        .map(|id| ModelInfo {
            id: (*id).to_string(),
            name: (*id).to_string(),
            owned_by: Some("anthropic".into()),
        })
        .collect())
}

async fn fetch_gemini_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<ModelInfo>, String> {
    let base = base_url.trim_end_matches('/');
    let mut url = format!("{base}/models");
    if !api_key.is_empty() {
        url.push_str(&format!("?key={}", urlencoding::encode(api_key)));
    }

    let response = client.get(&url).send().await;
    match response {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let mut models = Vec::new();
            if let Some(arr) = body["models"].as_array() {
                for item in arr {
                    if let Some(name) = item["name"].as_str() {
                        let id = name.trim_start_matches("models/");
                        let supports = item["supportedGenerationMethods"]
                            .as_array()
                            .is_none_or(|a| a.iter().any(|m| m.as_str() == Some("generateContent")));
                        if supports {
                            models.push(ModelInfo {
                                id: id.to_string(),
                                name: item["displayName"].as_str().unwrap_or(id).to_string(),
                                owned_by: Some("google".into()),
                            });
                        }
                    }
                }
            }
            if !models.is_empty() {
                models.sort_by(|a, b| a.id.cmp(&b.id));
                return Ok(models);
            }
        },
        _ => {},
    }

    Ok(GEMINI_FALLBACK
        .iter()
        .map(|id| ModelInfo {
            id: (*id).to_string(),
            name: (*id).to_string(),
            owned_by: Some("google".into()),
        })
        .collect())
}

/// 测试 LLM 连接
#[tauri::command]
pub async fn test_llm_connection(
    base_url: String,
    api_key: String,
    model: String,
    api_format: Option<String>,
) -> Result<String, String> {
    let format = normalize_format(api_format.as_deref());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    match format.as_str() {
        "anthropic" => test_anthropic(&client, &base_url, &api_key, &model).await,
        "gemini" => test_gemini(&client, &base_url, &api_key, &model).await,
        _ => test_openai(&client, &base_url, &api_key, &model).await,
    }
}

async fn test_openai(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "Hi"}],
        "max_tokens": 5,
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {status}: {body}"));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {e}"))?;

    let reply = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(空)");

    Ok(format!("连接成功！模型回复: {reply}"))
}

async fn test_anthropic(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    let url = format!("{}/messages", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "Hi"}]
    });
    let response = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("API 返回错误 {status}: {text}"));
    }
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("解析失败: {e}"))?;
    let mut reply = String::new();
    if let Some(arr) = v["content"].as_array() {
        for p in arr {
            if let Some(t) = p["text"].as_str() {
                reply.push_str(t);
            }
        }
    }
    Ok(format!(
        "连接成功！模型回复: {}",
        if reply.is_empty() { "(空)" } else { &reply }
    ))
}

async fn test_gemini(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    let base = base_url.trim_end_matches('/');
    let model = model.trim_start_matches("models/");
    let mut url = format!("{base}/models/{model}:generateContent");
    if !api_key.is_empty() {
        url.push_str(&format!("?key={}", urlencoding::encode(api_key)));
    }
    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": "Hi" }] }],
        "generationConfig": { "maxOutputTokens": 16 }
    });
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("API 返回错误 {status}: {text}"));
    }
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("解析失败: {e}"))?;
    let reply = v
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|t| t.as_str())
        .unwrap_or("(空)");
    Ok(format!("连接成功！模型回复: {reply}"))
}

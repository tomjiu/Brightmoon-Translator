// LLM Provider Management Commands - 多提供商管理 + 模型列表拉取

use serde::{Deserialize, Serialize};

/// 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub owned_by: Option<String>,
}

/// 模型列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

/// 从 API 端点拉取可用模型列表
#[tauri::command]
pub async fn fetch_available_models(
    base_url: String,
    api_key: String,
) -> Result<Vec<ModelInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {}: {}", status, body));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let mut models = Vec::new();

    // 兼容 OpenAI 格式 { data: [{ id, owned_by }] }
    if let Some(data) = body["data"].as_array() {
        for item in data {
            if let Some(id) = item["id"].as_str() {
                models.push(ModelInfo {
                    id: id.to_string(),
                    name: item["name"].as_str().unwrap_or(id).to_string(),
                    owned_by: item["owned_by"].as_str().map(|s| s.to_string()),
                });
            }
        }
    }
    // 兼容 Ollama 格式 { models: [{ name }] }
    else if let Some(data) = body["models"].as_array() {
        for item in data {
            if let Some(name) = item["name"].as_str() {
                models.push(ModelInfo {
                    id: name.to_string(),
                    name: name.to_string(),
                    owned_by: item["details"]["family"].as_str().map(|s| s.to_string()),
                });
            }
        }
    }

    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

/// 测试 LLM 连接
#[tauri::command]
pub async fn test_llm_connection(
    base_url: String,
    api_key: String,
    model: String,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "Hi"}],
        "max_tokens": 5,
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {}: {}", status, body));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let reply = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(空)");

    Ok(format!("✅ 连接成功！模型回复: {}", reply))
}

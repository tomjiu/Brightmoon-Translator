pub mod llm;
pub mod google;
pub mod baidu;
pub mod youdao;
pub mod deepl;
pub mod deeplx;
pub mod microsoft;
pub mod yandex;

use crate::config::AppConfig;
use crate::plugin;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A translation engine backed by an external plugin HTTP endpoint
pub struct PluginEngine {
    name: String,
    endpoint: String,
    headers: std::collections::HashMap<String, String>,
    client: Client,
}

impl PluginEngine {
    pub fn new(name: &str, endpoint: &str, headers: std::collections::HashMap<String, String>) -> Self {
        Self {
            name: name.to_string(),
            endpoint: endpoint.to_string(),
            headers,
            client: Client::new(),
        }
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }
}

#[async_trait]
impl TranslationEngine for PluginEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let mut req = self.client.post(&self.endpoint);

        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let body = serde_json::json!({
            "text": text,
            "from": from,
            "to": to,
        });

        let resp = req.json(&body).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Plugin returned status: {}", resp.status()));
        }

        let result: serde_json::Value = resp.json().await?;

        result
            .get("translated")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Plugin response missing 'translated' field"))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub engine: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResponse {
    pub results: Vec<TranslationResult>,
    pub detected_language: Option<String>,
}

#[async_trait]
pub trait TranslationEngine: Send + Sync {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String>;
    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn std::any::Any;
}

pub struct Router {
    engines: Vec<Arc<dyn TranslationEngine>>,
}

impl Router {
    pub fn new(config: &AppConfig) -> Self {
        let mut engines: Vec<Arc<dyn TranslationEngine>> = Vec::new();

        // Create shared HTTP client with proxy support
        let client = config.proxy.to_client_builder().build()
            .unwrap_or_else(|_| Client::new());

        // LLM engine (primary) - supports multiple API keys
        let llm_keys = config.llm.all_keys();
        if !llm_keys.is_empty() {
            let engine = llm::LlmEngine::with_multiple_keys(
                llm_keys,
                &config.llm.base_url,
                &config.llm.model,
            ).with_client(client.clone());
            let engine = if !config.custom_prompt.is_empty() {
                engine.with_custom_prompt(&config.custom_prompt)
            } else {
                engine
            };
            engines.push(Arc::new(engine));
        }

        // Google engine
        if config.engines.google.enabled {
            engines.push(Arc::new(google::GoogleEngine::new().with_client(client.clone())));
        }

        // Baidu engine
        if config.engines.baidu.enabled && !config.engines.baidu.app_id.is_empty() {
            engines.push(Arc::new(baidu::BaiduEngine::new(
                &config.engines.baidu.app_id,
                &config.engines.baidu.secret,
            ).with_client(client.clone())));
        }

        // Youdao engine (uses CDN-based key scraping, no API key needed)
        if config.engines.youdao.enabled {
            engines.push(Arc::new(youdao::YoudaoEngine::new().with_client(client.clone())));
        }

        // DeepL engine
        if config.engines.deepl.enabled && !config.engines.deepl.api_key.is_empty() {
            let engine = deepl::DeepLEngine::new(&config.engines.deepl.api_key).with_client(client.clone());
            let engine = if config.engines.deepl.pro {
                engine.with_pro()
            } else {
                engine
            };
            engines.push(Arc::new(engine));
        }

        // DeepLX engine (built-in, free DeepL alternative)
        if config.engines.deeplx.enabled {
            let mut engine = deeplx::DeepLXEngine::new()
                .with_client(client.clone());
            // If API key is provided, use Pro mode
            if let Some(ref key) = config.engines.deeplx.api_key {
                if !key.is_empty() {
                    engine = engine.with_api_key(key);
                    if config.engines.deeplx.pro {
                        engine = engine.with_pro(true);
                    }
                }
            }
            engines.push(Arc::new(engine));
        }

        // Microsoft engine (free, no config needed)
        if config.engines.microsoft.enabled {
            engines.push(Arc::new(microsoft::MicrosoftEngine::new().with_client(client.clone())));
        }

        // Yandex engine (free, no config needed)
        if config.engines.yandex.enabled {
            engines.push(Arc::new(yandex::YandexEngine::new().with_client(client.clone())));
        }

        // Fallback: if no engines configured, add a default LLM
        if engines.is_empty() {
            engines.push(Arc::new(llm::LlmEngine::new(
                "",
                "https://api.deepseek.com/v1",
                "deepseek-chat",
            ).with_client(client.clone())));
        }

        // Load plugin engines
        let plugins = plugin::scan_plugins();
        for p in &plugins {
            if p.manifest.enabled {
                if let Some(ref tc) = p.manifest.translation {
                    let engine = PluginEngine::new(
                        &format!("Plugin: {}", p.manifest.name),
                        &tc.endpoint,
                        tc.headers.clone(),
                    ).with_client(client.clone());
                    engines.push(Arc::new(engine));
                }
            }
        }

        Self { engines }
    }

    /// Rebuild engines list with new config (used when plugins change)
    pub fn rebuild(&self, config: &AppConfig) -> Self {
        Self::new(config)
    }

    pub async fn translate_all(&self, text: &str, from: &str, to: &str) -> TranslateResponse {
        let mut handles = Vec::new();

        for engine in &self.engines {
            let text = text.to_string();
            let from = from.to_string();
            let to = to.to_string();
            let engine = Arc::clone(engine);

            let handle = tokio::spawn(async move {
                let name = engine.name().to_string();
                match engine.translate(&text, &from, &to).await {
                    Ok(translated) => Some(TranslationResult {
                        engine: name,
                        text: translated,
                    }),
                    Err(e) => {
                        eprintln!("Engine {} error: {}", name, e);
                        None
                    }
                }
            });

            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(Some(result)) = handle.await {
                results.push(result);
            }
        }

        TranslateResponse {
            results,
            detected_language: None,
        }
    }

    pub async fn translate_primary(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        if let Some(engine) = self.engines.first() {
            engine.translate(text, from, to).await
        } else {
            Err(anyhow::anyhow!("No translation engine available"))
        }
    }

    pub fn engines_iter(&self) -> impl Iterator<Item = &Arc<dyn TranslationEngine>> {
        self.engines.iter()
    }

    pub fn engine_count(&self) -> usize {
        self.engines.len()
    }

    /// Translate with context from previous translations (for long document consistency)
    pub async fn translate_primary_with_context(
        &self,
        text: &str,
        from: &str,
        to: &str,
        context: &[llm::TranslationContext],
    ) -> anyhow::Result<String> {
        if let Some(engine) = self.engines.first() {
            // Try to downcast to LlmEngine for context support
            if let Some(llm_engine) = engine.as_any().downcast_ref::<llm::LlmEngine>() {
                llm_engine.translate_with_context(text, from, to, context).await
            } else {
                // Fallback to regular translate for non-LLM engines
                engine.translate(text, from, to).await
            }
        } else {
            Err(anyhow::anyhow!("No translation engine available"))
        }
    }

    /// Stream translation using primary engine, sending tokens via channel
    pub async fn translate_stream(
        &self,
        text: &str,
        from: &str,
        to: &str,
        tx: tokio::sync::mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        if let Some(engine) = self.engines.first() {
            // Try to downcast to LlmEngine for streaming support
            if let Some(llm_engine) = engine.as_any().downcast_ref::<llm::LlmEngine>() {
                llm_engine.translate_stream(text, from, to, tx).await
            } else {
                // Fallback: translate normally and send complete result
                let result = engine.translate(text, from, to).await?;
                let _ = tx.send(result.clone()).await;
                Ok(result)
            }
        } else {
            Err(anyhow::anyhow!("No translation engine available"))
        }
    }
}

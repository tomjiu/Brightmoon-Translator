pub mod baidu;
pub mod deepl;
pub mod deeplx;
pub mod google;
pub mod llm;
pub mod microsoft;
pub mod offline;
pub mod yandex;
pub mod youdao;

use crate::config::AppConfig;
use crate::plugin;
use async_trait::async_trait;
use reqwest::Client;
use std::sync::Arc;
use std::time::Instant;

// Re-export shared translation types from models
pub use crate::models::translation::{RoutingStrategy, TranslateResponse, TranslationResult};

/// Check HTTP response status. Returns error if not successful.
/// Borrows the response so the caller can still read the body afterward.
pub(crate) fn check_response(resp: &reqwest::Response, engine_name: &str) -> anyhow::Result<()> {
    let status = resp.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!("{} API error: {}", engine_name, status));
    }
    Ok(())
}

/// A translation engine backed by an external plugin HTTP endpoint
pub struct PluginEngine {
    name: String,
    endpoint: String,
    headers: std::collections::HashMap<String, String>,
    client: Client,
}

impl PluginEngine {
    pub fn new(
        name: &str,
        endpoint: &str,
        headers: std::collections::HashMap<String, String>,
    ) -> Self {
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
        check_response(&resp, "Plugin")?;

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

#[async_trait]
pub trait TranslationEngine: Send + Sync {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String>;
    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn std::any::Any;
}

struct EngineEntry {
    id: String,
    engine: Arc<dyn TranslationEngine>,
}

pub struct Router {
    engines: Vec<Arc<dyn TranslationEngine>>,
    strategy: RoutingStrategy,
}

impl Router {
    pub fn new(config: &AppConfig) -> Self {
        let mut available: Vec<EngineEntry> = Vec::new();

        // Create shared HTTP client with proxy support and configurable timeout
        let client = config
            .proxy
            .to_client_builder()
            .timeout(std::time::Duration::from_secs(config.http_timeout_secs))
            .build()
            .unwrap_or_else(|_| Client::new());

        // LLM client with longer timeout for LLM requests
        let llm_client = config
            .proxy
            .to_client_builder()
            .timeout(std::time::Duration::from_secs(config.llm_timeout_secs))
            .build()
            .unwrap_or_else(|_| Client::new());

        // LLM engine (primary) - supports multiple API keys
        let llm_keys = config.llm.all_keys();
        if !llm_keys.is_empty() {
            let engine = llm::LlmEngine::with_multiple_keys(
                llm_keys,
                &config.llm.base_url,
                &config.llm.model,
            )
            .with_client(llm_client.clone());
            let engine = if !config.custom_prompt.is_empty() {
                engine.with_custom_prompt(&config.custom_prompt)
            } else {
                engine
            };
            available.push(EngineEntry {
                id: "llm".to_string(),
                engine: Arc::new(engine),
            });
        }

        // Youdao engine (free, no API key needed) - prioritize over Google
        if config.engines.youdao.enabled {
            available.push(EngineEntry {
                id: "youdao".to_string(),
                engine: Arc::new(youdao::YoudaoEngine::new().with_client(client.clone())),
            });
        }

        // DeepL engine (requires API key)
        if config.engines.deepl.enabled && !config.engines.deepl.api_key.is_empty() {
            let engine =
                deepl::DeepLEngine::new(&config.engines.deepl.api_key).with_client(client.clone());
            let engine = if config.engines.deepl.pro {
                engine.with_pro()
            } else {
                engine
            };
            available.push(EngineEntry {
                id: "deepl".to_string(),
                engine: Arc::new(engine),
            });
        }

        // DeepLX engine (built-in, free DeepL alternative)
        if config.engines.deeplx.enabled {
            let mut engine = deeplx::DeepLXEngine::new().with_client(client.clone());
            // If API key is provided, use Pro mode
            if let Some(ref key) = config.engines.deeplx.api_key {
                if !key.is_empty() {
                    engine = engine.with_api_key(key);
                    if config.engines.deeplx.pro {
                        engine = engine.with_pro(true);
                    }
                }
            }
            available.push(EngineEntry {
                id: "deeplx".to_string(),
                engine: Arc::new(engine),
            });
        }

        // Baidu engine (requires API key)
        if config.engines.baidu.enabled && !config.engines.baidu.app_id.is_empty() {
            available.push(EngineEntry {
                id: "baidu".to_string(),
                engine: Arc::new(
                    baidu::BaiduEngine::new(
                        &config.engines.baidu.app_id,
                        &config.engines.baidu.secret,
                    )
                    .with_client(client.clone()),
                ),
            });
        }

        // Microsoft engine (free, no config needed)
        if config.engines.microsoft.enabled {
            available.push(EngineEntry {
                id: "microsoft".to_string(),
                engine: Arc::new(microsoft::MicrosoftEngine::new().with_client(client.clone())),
            });
        }

        // Yandex engine (free, no config needed)
        if config.engines.yandex.enabled {
            available.push(EngineEntry {
                id: "yandex".to_string(),
                engine: Arc::new(yandex::YandexEngine::new().with_client(client.clone())),
            });
        }

        // Google engine (free, no config needed) - lowest priority
        if config.engines.google.enabled {
            available.push(EngineEntry {
                id: "google".to_string(),
                engine: Arc::new(google::GoogleEngine::new().with_client(client.clone())),
            });
        }

        // Offline engine (local translation models)
        if config.engines.offline.enabled {
            let model_dir = if config.engines.offline.model_dir.is_empty() {
                None
            } else {
                Some(config.engines.offline.model_dir.as_str())
            };
            let offline_engine = offline::OfflineEngine::new(model_dir);
            available.push(EngineEntry {
                id: "offline".to_string(),
                engine: Arc::new(offline_engine),
            });
        }

        // Load plugin engines
        let plugins = plugin::scan_plugins();
        for p in &plugins {
            if p.manifest.enabled {
                if let Some(ref tc) = p.manifest.translation {
                    let plugin_id = format!(
                        "plugin_{}",
                        p.manifest.name.to_lowercase().replace(' ', "_")
                    );
                    let engine = PluginEngine::new(
                        &format!("Plugin: {}", p.manifest.name),
                        &tc.endpoint,
                        tc.headers.clone(),
                    )
                    .with_client(client.clone());
                    available.push(EngineEntry {
                        id: plugin_id,
                        engine: Arc::new(engine),
                    });
                }
            }
        }

        // Order engines according to config
        let engines = order_engines(available, &[]);

        // Log configured engines for debugging
        if engines.is_empty() {
            tracing::error!("[Router] No translation engines available - check configuration");
        }
        let engine_names: Vec<&str> = engines.iter().map(|e| e.name()).collect();
        tracing::info!(
            "[Router] Configured engines: {:?} (strategy: {:?})",
            engine_names,
            config.routing_strategy
        );

        Self {
            engines,
            strategy: config.routing_strategy.clone().unwrap_or_default(),
        }
    }

    /// Get the list of available engine names
    pub fn engine_names(&self) -> Vec<&str> {
        self.engines.iter().map(|e| e.name()).collect()
    }

    /// Get the primary engine's name
    pub fn primary_engine_name(&self) -> Option<&str> {
        self.engines.first().map(|e| e.name())
    }

    /// Rebuild engines list with new config (used when plugins change)
    pub fn rebuild(&self, config: &AppConfig) -> Self {
        Self::new(config)
    }

    pub async fn translate_all(&self, text: &str, from: &str, to: &str) -> TranslateResponse {
        match self.strategy {
            RoutingStrategy::PrimaryOnly => self.translate_primary_only(text, from, to).await,
            RoutingStrategy::FallbackOnError => self.translate_with_fallback(text, from, to).await,
            RoutingStrategy::ParallelCompare => {
                self.translate_parallel_compare(text, from, to).await
            },
            RoutingStrategy::CostAware => self.translate_cost_aware(text, from, to).await,
            RoutingStrategy::LatencyFirst => self.translate_latency_first(text, from, to).await,
        }
    }

    /// Translate with all engines except the primary (for when primary is handled separately)
    pub async fn translate_rest(&self, text: &str, from: &str, to: &str) -> TranslateResponse {
        let mut results = Vec::new();
        let engines: Vec<_> = self.engines.iter().skip(1).collect();

        if engines.is_empty() {
            return TranslateResponse {
                results,
                detected_language: None,
            };
        }

        let text: Arc<str> = Arc::from(text);
        let from: Arc<str> = Arc::from(from);
        let to: Arc<str> = Arc::from(to);

        let mut handles = Vec::new();
        for engine in engines {
            let engine = engine.clone();
            let text = Arc::clone(&text);
            let from = Arc::clone(&from);
            let to = Arc::clone(&to);
            handles.push(tokio::spawn(async move {
                let name = engine.name().to_string();
                match engine.translate(&text, &from, &to).await {
                    Ok(translated) => Some(TranslationResult {
                        engine: name,
                        text: translated,
                        latency_ms: None,
                    }),
                    Err(e) => {
                        tracing::warn!("[Router] Engine {} failed: {}", name, e);
                        None
                    },
                }
            }));
        }

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

    /// Strategy: Primary Only - use first engine only
    async fn translate_primary_only(&self, text: &str, from: &str, to: &str) -> TranslateResponse {
        if let Some(engine) = self.engines.first() {
            let name = engine.name().to_string();
            tracing::info!("[Router] Using primary engine: {}", name);
            match engine.translate(text, from, to).await {
                Ok(translated) => {
                    tracing::info!("[Router] Primary engine {} succeeded", name);
                    TranslateResponse {
                        results: vec![TranslationResult {
                            engine: name,
                            text: translated,
                            latency_ms: None,
                        }],
                        detected_language: None,
                    }
                },
                Err(e) => {
                    tracing::error!("[Router] Primary engine {} failed: {}", name, e);
                    TranslateResponse {
                        results: vec![],
                        detected_language: None,
                    }
                },
            }
        } else {
            tracing::error!("[Router] No engines configured");
            TranslateResponse {
                results: vec![],
                detected_language: None,
            }
        }
    }

    /// Strategy: Fallback on Error - try each engine until one succeeds
    async fn translate_with_fallback(&self, text: &str, from: &str, to: &str) -> TranslateResponse {
        for engine in &self.engines {
            let name = engine.name().to_string();
            tracing::info!("[Router] Trying engine: {}", name);
            match engine.translate(text, from, to).await {
                Ok(translated) => {
                    tracing::info!("[Router] Engine {} succeeded", name);
                    return TranslateResponse {
                        results: vec![TranslationResult {
                            engine: name,
                            text: translated,
                            latency_ms: None,
                        }],
                        detected_language: None,
                    };
                },
                Err(e) => {
                    tracing::warn!("[Router] Engine {} failed: {}, trying next...", name, e);
                    continue;
                },
            }
        }

        tracing::error!("[Router] All engines failed");
        TranslateResponse {
            results: vec![],
            detected_language: None,
        }
    }

    /// Strategy: Parallel Compare - run all engines, return all results
    pub async fn translate_parallel_compare(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> TranslateResponse {
        let text: Arc<str> = Arc::from(text);
        let from: Arc<str> = Arc::from(from);
        let to: Arc<str> = Arc::from(to);

        let mut handles = Vec::new();

        for engine in &self.engines {
            let text = Arc::clone(&text);
            let from = Arc::clone(&from);
            let to = Arc::clone(&to);
            let engine = Arc::clone(engine);

            let handle = tokio::spawn(async move {
                let name = engine.name().to_string();
                match engine.translate(&text, &from, &to).await {
                    Ok(translated) => Some(TranslationResult {
                        engine: name,
                        text: translated,
                        latency_ms: None,
                    }),
                    Err(e) => {
                        tracing::warn!("Engine {} error: {}", name, e);
                        None
                    },
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

    /// Strategy: Cost Aware - prefer free engines (Google, Youdao, DeepLX, Offline)
    async fn translate_cost_aware(&self, text: &str, from: &str, to: &str) -> TranslateResponse {
        const FREE_ENGINES: &[&str] = &["Google", "Youdao", "DeepLX", "Offline"];

        // Try free engines first
        for engine in &self.engines {
            if FREE_ENGINES.contains(&engine.name()) {
                let name = engine.name().to_string();
                match engine.translate(text, from, to).await {
                    Ok(translated) => {
                        return TranslateResponse {
                            results: vec![TranslationResult {
                                engine: name,
                                text: translated,
                                latency_ms: None,
                            }],
                            detected_language: None,
                        };
                    },
                    Err(e) => {
                        tracing::warn!("Free engine {} failed: {}", name, e);
                        continue;
                    },
                }
            }
        }

        // Fallback to paid engines
        for engine in &self.engines {
            if !FREE_ENGINES.contains(&engine.name()) {
                let name = engine.name().to_string();
                match engine.translate(text, from, to).await {
                    Ok(translated) => {
                        return TranslateResponse {
                            results: vec![TranslationResult {
                                engine: name,
                                text: translated,
                                latency_ms: None,
                            }],
                            detected_language: None,
                        };
                    },
                    Err(e) => {
                        tracing::warn!("Paid engine {} failed: {}", name, e);
                        continue;
                    },
                }
            }
        }

        TranslateResponse {
            results: vec![],
            detected_language: None,
        }
    }

    /// Strategy: Latency First - run all in parallel, return first success
    async fn translate_latency_first(&self, text: &str, from: &str, to: &str) -> TranslateResponse {
        let text: Arc<str> = Arc::from(text);
        let from: Arc<str> = Arc::from(from);
        let to: Arc<str> = Arc::from(to);

        let mut handles = Vec::new();

        for engine in &self.engines {
            let text = Arc::clone(&text);
            let from = Arc::clone(&from);
            let to = Arc::clone(&to);
            let engine = Arc::clone(engine);

            let handle = tokio::spawn(async move {
                let start = Instant::now();
                let name = engine.name().to_string();
                match engine.translate(&text, &from, &to).await {
                    Ok(translated) => {
                        let elapsed = start.elapsed();
                        Some(TranslationResult {
                            engine: name,
                            text: translated,
                            latency_ms: Some(elapsed.as_millis() as u64),
                        })
                    },
                    Err(e) => {
                        tracing::warn!("Engine {} error: {}", name, e);
                        None
                    },
                }
            });

            handles.push(handle);
        }

        // Use select to get first completed result (not sequential await)
        let mut remaining = handles;

        while !remaining.is_empty() {
            let (result, _index, rest) = futures::future::select_all(remaining).await;
            remaining = rest;

            if let Ok(Some(translation_result)) = result {
                return TranslateResponse {
                    results: vec![translation_result],
                    detected_language: None,
                };
            }
        }

        TranslateResponse {
            results: vec![],
            detected_language: None,
        }
    }

    pub async fn translate_primary(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<String> {
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

    /// Translate with glossary terms injected into the LLM system prompt
    pub async fn translate_primary_with_glossary(
        &self,
        text: &str,
        from: &str,
        to: &str,
        glossary_hint: &str,
    ) -> anyhow::Result<String> {
        if let Some(engine) = self.engines.first() {
            if let Some(llm_engine) = engine.as_any().downcast_ref::<llm::LlmEngine>() {
                llm_engine
                    .translate_with_glossary(text, from, to, glossary_hint)
                    .await
            } else {
                engine.translate(text, from, to).await
            }
        } else {
            Err(anyhow::anyhow!("No translation engine available"))
        }
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
                llm_engine
                    .translate_with_context(text, from, to, context)
                    .await
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

    /// Stream translation with glossary hint injected into the LLM system prompt
    pub async fn translate_stream_with_glossary(
        &self,
        text: &str,
        from: &str,
        to: &str,
        tx: tokio::sync::mpsc::Sender<String>,
        glossary_hint: &str,
    ) -> anyhow::Result<String> {
        if let Some(engine) = self.engines.first() {
            if let Some(llm_engine) = engine.as_any().downcast_ref::<llm::LlmEngine>() {
                llm_engine
                    .translate_stream_with_glossary(text, from, to, tx, glossary_hint)
                    .await
            } else {
                let result = engine.translate(text, from, to).await?;
                let _ = tx.send(result.clone()).await;
                Ok(result)
            }
        } else {
            Err(anyhow::anyhow!("No translation engine available"))
        }
    }
}

/// Order engines according to configured priority
fn order_engines(
    mut available: Vec<EngineEntry>,
    configured_order: &[String],
) -> Vec<Arc<dyn TranslationEngine>> {
    let mut ordered = Vec::with_capacity(available.len());

    // Add engines in configured order
    for requested_id in configured_order {
        if let Some(index) = available
            .iter()
            .position(|entry| entry.id.eq_ignore_ascii_case(requested_id))
        {
            ordered.push(available.remove(index).engine);
        }
    }

    // Add remaining engines in discovery order
    ordered.extend(available.into_iter().map(|entry| entry.engine));
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn router_with_no_engines_returns_empty_results() {
        let mut config = AppConfig::default();
        config.engines.google.enabled = false;
        config.engines.youdao.enabled = false;
        config.engines.deepl.enabled = false;
        config.engines.deeplx.enabled = false;
        config.engines.baidu.enabled = false;
        config.engines.microsoft.enabled = false;
        config.engines.yandex.enabled = false;
        config.engines.offline.enabled = false;

        let router = Router::new(&config);
        assert!(router.engines.is_empty());
    }

    #[tokio::test]
    async fn empty_router_returns_empty_response() {
        let router = Router {
            engines: vec![],
            strategy: RoutingStrategy::PrimaryOnly,
        };

        let response = router.translate_all("hello", "en", "zh").await;
        assert!(response.results.is_empty());
    }

    #[test]
    fn order_engines_respects_configured_order() {
        use crate::engine::google;
        use crate::engine::youdao;

        let entries = vec![
            EngineEntry {
                id: "google".to_string(),
                engine: Arc::new(google::GoogleEngine::new()),
            },
            EngineEntry {
                id: "youdao".to_string(),
                engine: Arc::new(youdao::YoudaoEngine::new()),
            },
        ];

        let ordered = order_engines(entries, &["youdao".to_string(), "google".to_string()]);

        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].name(), "Youdao");
        assert_eq!(ordered[1].name(), "Google");
    }
}

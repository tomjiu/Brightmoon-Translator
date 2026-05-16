pub mod baidu;
pub mod deepl;
pub mod deeplx;
pub mod google;
pub mod llm;
pub mod microsoft;
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

#[async_trait]
pub trait TranslationEngine: Send + Sync {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String>;
    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn std::any::Any;
}

pub struct Router {
    engines: Vec<Arc<dyn TranslationEngine>>,
    strategy: RoutingStrategy,
}

impl Router {
    pub fn new(config: &AppConfig) -> Self {
        let mut engines: Vec<Arc<dyn TranslationEngine>> = Vec::new();

        // Create shared HTTP client with proxy support
        let client = config
            .proxy
            .to_client_builder()
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
            .with_client(client.clone());
            let engine = if !config.custom_prompt.is_empty() {
                engine.with_custom_prompt(&config.custom_prompt)
            } else {
                engine
            };
            engines.push(Arc::new(engine));
        }

        // Youdao engine (free, no API key needed) - prioritize over Google
        if config.engines.youdao.enabled {
            engines.push(Arc::new(
                youdao::YoudaoEngine::new().with_client(client.clone()),
            ));
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
            engines.push(Arc::new(engine));
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
            engines.push(Arc::new(engine));
        }

        // Baidu engine (requires API key)
        if config.engines.baidu.enabled && !config.engines.baidu.app_id.is_empty() {
            engines.push(Arc::new(
                baidu::BaiduEngine::new(&config.engines.baidu.app_id, &config.engines.baidu.secret)
                    .with_client(client.clone()),
            ));
        }

        // Microsoft engine (free, no config needed)
        if config.engines.microsoft.enabled {
            engines.push(Arc::new(
                microsoft::MicrosoftEngine::new().with_client(client.clone()),
            ));
        }

        // Yandex engine (free, no config needed)
        if config.engines.yandex.enabled {
            engines.push(Arc::new(
                yandex::YandexEngine::new().with_client(client.clone()),
            ));
        }

        // Google engine (free, no config needed) - lowest priority
        if config.engines.google.enabled {
            engines.push(Arc::new(
                google::GoogleEngine::new().with_client(client.clone()),
            ));
        }

        // Fallback: if no engines configured, add a default LLM
        if engines.is_empty() {
            engines.push(Arc::new(
                llm::LlmEngine::new("", "https://api.deepseek.com/v1", "deepseek-chat")
                    .with_client(client.clone()),
            ));
        }

        // Log configured engines for debugging
        let engine_names: Vec<&str> = engines.iter().map(|e| e.name()).collect();
        log::info!("[Router] Configured engines: {:?} (strategy: {:?})", engine_names, config.routing_strategy);

        // Load plugin engines
        let plugins = plugin::scan_plugins();
        for p in &plugins {
            if p.manifest.enabled {
                if let Some(ref tc) = p.manifest.translation {
                    let engine = PluginEngine::new(
                        &format!("Plugin: {}", p.manifest.name),
                        &tc.endpoint,
                        tc.headers.clone(),
                    )
                    .with_client(client.clone());
                    engines.push(Arc::new(engine));
                }
            }
        }

        Self {
            engines,
            strategy: config.routing_strategy.clone().unwrap_or_default(),
        }
    }

    /// Get the list of available engine names
    pub fn engine_names(&self) -> Vec<String> {
        self.engines.iter().map(|e| e.name().to_string()).collect()
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
            }
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

        let mut handles = Vec::new();
        for engine in engines {
            let engine = engine.clone();
            let text = text.to_string();
            let from = from.to_string();
            let to = to.to_string();
            handles.push(tokio::spawn(async move {
                let name = engine.name().to_string();
                match engine.translate(&text, &from, &to).await {
                    Ok(translated) => Some(TranslationResult {
                        engine: name,
                        text: translated,
                        latency_ms: None,
                    }),
                    Err(e) => {
                        log::warn!("[Router] Engine {} failed: {}", name, e);
                        None
                    }
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
            log::info!("[Router] Using primary engine: {}", name);
            match engine.translate(text, from, to).await {
                Ok(translated) => {
                    log::info!("[Router] Primary engine {} succeeded", name);
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
                    log::error!("[Router] Primary engine {} failed: {}", name, e);
                    TranslateResponse {
                        results: vec![],
                        detected_language: None,
                    }
                }
            }
        } else {
            log::error!("[Router] No engines configured");
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
            log::info!("[Router] Trying engine: {}", name);
            match engine.translate(text, from, to).await {
                Ok(translated) => {
                    log::info!("[Router] Engine {} succeeded", name);
                    return TranslateResponse {
                        results: vec![TranslationResult {
                            engine: name,
                            text: translated,
                            latency_ms: None,
                        }],
                        detected_language: None,
                    };
                }
                Err(e) => {
                    log::warn!("[Router] Engine {} failed: {}, trying next...", name, e);
                    continue;
                }
            }
        }

        log::error!("[Router] All engines failed");
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
                        latency_ms: None,
                    }),
                    Err(e) => {
                        log::warn!("Engine {} error: {}", name, e);
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

    /// Strategy: Cost Aware - prefer free engines (Google, Microsoft, Yandex, DeepLX)
    async fn translate_cost_aware(&self, text: &str, from: &str, to: &str) -> TranslateResponse {
        let free_engines: Vec<&str> = vec!["Google", "Microsoft", "Yandex", "DeepLX"];

        // Try free engines first
        for engine in &self.engines {
            if free_engines.contains(&engine.name()) {
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
                    }
                    Err(e) => {
                        log::warn!("Free engine {} failed: {}", name, e);
                        continue;
                    }
                }
            }
        }

        // Fallback to paid engines
        for engine in &self.engines {
            if !free_engines.contains(&engine.name()) {
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
                    }
                    Err(e) => {
                        log::warn!("Paid engine {} failed: {}", name, e);
                        continue;
                    }
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
        let mut handles = Vec::new();

        for engine in &self.engines {
            let text = text.to_string();
            let from = from.to_string();
            let to = to.to_string();
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
                    }
                    Err(e) => {
                        log::warn!("Engine {} error: {}", name, e);
                        None
                    }
                }
            });

            handles.push(handle);
        }

        // Use select to get first completed result (not sequential await)
        let results = Vec::new();
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
            results,
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

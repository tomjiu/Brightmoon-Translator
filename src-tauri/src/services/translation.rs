use crate::blacklist::BlacklistProcessor;
use crate::cache::TranslationCache;
use crate::config::AppConfig;
use crate::engine::{llm::TranslationContext, Router, TranslateResponse, TranslationResult};
use crate::glossary::Glossary;
use crate::memory::HistoryStore;
use crate::metrics::MetricsCollector;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

/// Result for a single line in batch translation
#[derive(Debug, Clone)]
pub struct BatchTranslationResult {
    pub index: usize,
    pub original: String,
    pub translated: String,
}

/// Service layer for translation operations
/// Handles glossary, blacklist, cache, history, and engine orchestration
pub struct TranslationService {
    config: Arc<Mutex<AppConfig>>,
    glossary: Arc<Mutex<Glossary>>,
    history: Arc<Mutex<HistoryStore>>,
    cache: Arc<TranslationCache>,
    engine_router: Arc<RwLock<Router>>,
    metrics: Arc<MetricsCollector>,
}

impl TranslationService {
    pub fn new(
        config: Arc<Mutex<AppConfig>>,
        glossary: Arc<Mutex<Glossary>>,
        history: Arc<Mutex<HistoryStore>>,
        cache: Arc<TranslationCache>,
        engine_router: Arc<RwLock<Router>>,
        metrics: Arc<MetricsCollector>,
    ) -> Self {
        Self {
            config,
            glossary,
            history,
            cache,
            engine_router,
            metrics,
        }
    }

    /// Translate text with full pipeline: glossary -> blacklist -> cache -> engine -> restore -> cache -> history
    pub async fn translate(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslateResponse, String> {
        // Apply glossary
        let glossary = self.glossary.lock().await;
        let mut processed_text = text.to_string();
        let lang_pair = format!("{}-{}", from, to);
        glossary.apply_glossary(&mut processed_text, &lang_pair);
        drop(glossary);

        // Apply blacklist protection
        let config = self.config.lock().await;
        let blacklist_processor = BlacklistProcessor::new(config.translation_blacklist.clone());
        drop(config);

        let (protected_text, placeholder_map) = blacklist_processor.protect(&processed_text);
        let has_blacklist = !placeholder_map.is_empty();

        // Check cache first
        if let Some(cached) = self.cache.get(&protected_text, from, to).await {
            self.metrics.record_cache_hit().await;
            let results = cached
                .results
                .into_iter()
                .map(|(engine, text)| {
                    let final_text = if has_blacklist {
                        blacklist_processor.restore(&text, &placeholder_map)
                    } else {
                        text
                    };
                    TranslationResult {
                        engine,
                        text: final_text,
                    }
                })
                .collect();
            return Ok(TranslateResponse {
                results,
                detected_language: None,
            });
        }
        self.metrics.record_cache_miss().await;

        // Call translation engines with timing
        let start = Instant::now();
        let router = self.engine_router.read().await;
        let mut response = router.translate_all(&protected_text, from, to).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        drop(router);

        // Record engine latency for each result
        for result in &response.results {
            self.metrics.record_engine_latency(&result.engine, elapsed_ms).await;
        }

        // Record failures for empty results
        if response.results.is_empty() {
            self.metrics.record_failure("all", "No engine returned a result").await;
        }

        // Restore blacklist words in results
        if has_blacklist {
            for result in &mut response.results {
                result.text = blacklist_processor.restore(&result.text, &placeholder_map);
            }
        }

        // Cache the results
        if !response.results.is_empty() {
            let cache_results: Vec<(String, String)> = response
                .results
                .iter()
                .map(|r| (r.engine.clone(), r.text.clone()))
                .collect();
            self.cache
                .set(&protected_text, from, to, cache_results)
                .await;
        }

        // Save to history
        if let Some(first) = response.results.first() {
            let history = self.history.lock().await;
            history.add(text, &first.text, from, to, &first.engine);
        }

        Ok(response)
    }

    /// Stream translation using primary engine
    pub async fn translate_stream(
        &self,
        text: &str,
        from: &str,
        to: &str,
        tx: tokio::sync::mpsc::Sender<String>,
    ) -> Result<String, String> {
        // Apply glossary
        let glossary = self.glossary.lock().await;
        let mut processed_text = text.to_string();
        let lang_pair = format!("{}-{}", from, to);
        glossary.apply_glossary(&mut processed_text, &lang_pair);
        drop(glossary);

        // Check cache first
        if let Some(cached) = self.cache.get(&processed_text, from, to).await {
            if let Some((_, cached_text)) = cached.results.first() {
                self.metrics.record_cache_hit().await;
                let _ = tx.send(cached_text.clone()).await;
                return Ok(cached_text.clone());
            }
        }
        self.metrics.record_cache_miss().await;

        // Stream translation using primary engine
        let start = Instant::now();
        let router = self.engine_router.read().await;
        let result = router
            .translate_stream(&processed_text, from, to, tx)
            .await;
        drop(router);

        match result {
            Ok(full_text) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                self.metrics.record_engine_latency("LLM", elapsed_ms).await;

                // Cache the result
                if !full_text.is_empty() {
                    self.cache
                        .set(
                            &processed_text,
                            from,
                            to,
                            vec![("LLM".to_string(), full_text.clone())],
                        )
                        .await;

                    // Save to history
                    let history = self.history.lock().await;
                    history.add(text, &full_text, from, to, "LLM");
                }

                Ok(full_text)
            }
            Err(e) => {
                self.metrics.record_failure("LLM", &e.to_string()).await;
                Err(format!("Streaming failed: {}", e))
            }
        }
    }

    /// Translate with primary engine only (for quick translations)
    pub async fn translate_primary(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, String> {
        // Apply glossary
        let glossary = self.glossary.lock().await;
        let mut processed_text = text.to_string();
        let lang_pair = format!("{}-{}", from, to);
        glossary.apply_glossary(&mut processed_text, &lang_pair);
        drop(glossary);

        let start = Instant::now();
        let router = self.engine_router.read().await;
        let result = router
            .translate_primary(&processed_text, from, to)
            .await;
        drop(router);

        match result {
            Ok(translated) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                self.metrics.record_engine_latency("primary", elapsed_ms).await;
                Ok(translated)
            }
            Err(e) => {
                self.metrics.record_failure("primary", &e.to_string()).await;
                Err(e.to_string())
            }
        }
    }

    /// Translate with context for document consistency
    pub async fn translate_with_context(
        &self,
        text: &str,
        from: &str,
        to: &str,
        context: &[crate::engine::llm::TranslationContext],
    ) -> Result<String, String> {
        let router = self.engine_router.read().await;
        router
            .translate_primary_with_context(text, from, to, context)
            .await
            .map_err(|e| e.to_string())
    }

    /// Get the engine router for advanced operations
    pub fn router(&self) -> &Arc<RwLock<Router>> {
        &self.engine_router
    }

    /// Batch translate multiple lines with concurrency control and context reuse
    /// Returns results in the same order as input
    pub async fn translate_batch(
        &self,
        lines: &[(usize, &str)], // (original_index, text)
        from: &str,
        to: &str,
        concurrency: usize,
    ) -> Vec<BatchTranslationResult> {
        if lines.is_empty() {
            return Vec::new();
        }

        let concurrency = concurrency.max(1).min(10); // Clamp to 1-10
        let mut results = Vec::with_capacity(lines.len());
        let mut context: Vec<TranslationContext> = Vec::new();

        // Process in chunks with concurrency
        for chunk in lines.chunks(concurrency) {
            let mut handles = Vec::new();

            for &(idx, text) in chunk {
                let text = text.to_string();
                let from = from.to_string();
                let to = to.to_string();
                let context_snapshot = context.clone();
                let router = self.engine_router.clone();

                let handle = tokio::spawn(async move {
                    let router = router.read().await;
                    let translated = router
                        .translate_primary_with_context(&text, &from, &to, &context_snapshot)
                        .await
                        .unwrap_or_default();
                    drop(router);

                    BatchTranslationResult {
                        index: idx,
                        original: text,
                        translated,
                    }
                });

                handles.push(handle);
            }

            // Wait for all in this chunk
            for handle in handles {
                if let Ok(result) = handle.await {
                    // Add to context for consistency (keep last 5)
                    context.push(TranslationContext {
                        source: result.original.clone(),
                        translation: result.translated.clone(),
                    });
                    if context.len() > 5 {
                        context.remove(0);
                    }
                    results.push(result);
                }
            }
        }

        // Sort by original index to maintain order
        results.sort_by_key(|r| r.index);
        results
    }

    /// Translate text lines for embedded/subtitle with progress callback
    pub async fn translate_embedded_batch<F>(
        &self,
        text: &str,
        from: &str,
        to: &str,
        concurrency: usize,
        mut on_progress: F,
    ) -> Vec<BatchTranslationResult>
    where
        F: FnMut(usize, usize),
    {
        let lines: Vec<(usize, &str)> = text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
            .map(|(i, l)| (i, l.trim()))
            .collect();

        let total = lines.len();
        if total == 0 {
            return Vec::new();
        }

        let concurrency = concurrency.max(1).min(10);
        let mut results = Vec::with_capacity(total);
        let mut context: Vec<TranslationContext> = Vec::new();
        let mut completed = 0;

        // Process in chunks with concurrency
        for chunk in lines.chunks(concurrency) {
            let mut handles = Vec::new();

            for &(idx, text) in chunk {
                let text = text.to_string();
                let from = from.to_string();
                let to = to.to_string();
                let context_snapshot = context.clone();
                let router = self.engine_router.clone();

                let handle = tokio::spawn(async move {
                    let router = router.read().await;
                    let translated = router
                        .translate_primary_with_context(&text, &from, &to, &context_snapshot)
                        .await
                        .unwrap_or_default();
                    drop(router);

                    BatchTranslationResult {
                        index: idx,
                        original: text,
                        translated,
                    }
                });

                handles.push(handle);
            }

            // Wait for all in this chunk
            for handle in handles {
                if let Ok(result) = handle.await {
                    // Add to context for consistency (keep last 5)
                    context.push(TranslationContext {
                        source: result.original.clone(),
                        translation: result.translated.clone(),
                    });
                    if context.len() > 5 {
                        context.remove(0);
                    }
                    results.push(result);
                    completed += 1;
                    on_progress(completed, total);
                }
            }
        }

        // Sort by original index to maintain order
        results.sort_by_key(|r| r.index);
        results
    }
}

use crate::blacklist::BlacklistProcessor;
use crate::cache::TranslationCache;
use crate::config::AppConfig;
use crate::engine::{llm::TranslationContext, Router, TranslateResponse, TranslationResult};
use crate::glossary::Glossary;
use crate::memory::HistoryStore;
use crate::metrics::MetricsCollector;
use crate::models::error::TranslationError;
use crate::post_process::PostProcessor;
use crate::pre_process::PreProcessor;
pub use crate::models::translation::BatchTranslationResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

/// Result of text preparation before engine call
struct PreparedText {
    text: String,
    glossary_hint: String,
    blacklist: Option<(BlacklistProcessor, HashMap<String, String>)>,
}

/// Service layer for translation operations
/// Handles pre-processing, glossary, blacklist, cache, history, and engine orchestration
pub struct TranslationService {
    config: Arc<Mutex<AppConfig>>,
    glossary: Arc<Mutex<Glossary>>,
    history: Arc<Mutex<HistoryStore>>,
    cache: Arc<TranslationCache>,
    engine_router: Arc<RwLock<Router>>,
    metrics: Arc<MetricsCollector>,
    pre_processor: Arc<PreProcessor>,
    post_processor: Arc<Mutex<PostProcessor>>,
}

impl TranslationService {
    pub fn new(
        config: Arc<Mutex<AppConfig>>,
        glossary: Arc<Mutex<Glossary>>,
        history: Arc<Mutex<HistoryStore>>,
        cache: Arc<TranslationCache>,
        engine_router: Arc<RwLock<Router>>,
        metrics: Arc<MetricsCollector>,
        pre_processor: Arc<PreProcessor>,
        post_processor: Arc<Mutex<PostProcessor>>,
    ) -> Self {
        Self {
            config,
            glossary,
            history,
            cache,
            engine_router,
            metrics,
            pre_processor,
            post_processor,
        }
    }

    /// Prepare text for translation: pre-process -> glossary -> blacklist protect
    async fn prepare(&self, text: &str, from: &str, to: &str) -> PreparedText {
        let lang_pair = format!("{}-{}", from, to);
        let mut processed_text = self.pre_processor.process(text, Some(&lang_pair));

        let glossary = self.glossary.lock().await;
        glossary.apply_glossary(&mut processed_text, &lang_pair);
        let glossary_hint = glossary.format_hint(&lang_pair);
        drop(glossary);

        let config = self.config.lock().await;
        let blacklist_processor = BlacklistProcessor::new(config.translation_blacklist.clone());
        drop(config);

        let (protected_text, placeholder_map) = blacklist_processor.protect(&processed_text);

        PreparedText {
            text: protected_text,
            glossary_hint,
            blacklist: if placeholder_map.is_empty() {
                None
            } else {
                Some((blacklist_processor, placeholder_map))
            },
        }
    }

    /// Finalize single translation result: restore blacklist -> post-process -> auto-correct
    async fn finalize(
        &self,
        translated: &str,
        source: &str,
        from: &str,
        to: &str,
        blacklist: &Option<(BlacklistProcessor, HashMap<String, String>)>,
    ) -> String {
        let restored = if let Some((bp, pm)) = blacklist {
            bp.restore(translated, pm)
        } else {
            translated.to_string()
        };

        let pp = self.post_processor.lock().await;
        let processed = pp.process(&restored);
        let ac = pp.auto_correct(&processed, source, from, to);
        if !ac.warnings.is_empty() {
            tracing::warn!("[Translation] Auto-correct warnings: {:?}", ac.warnings);
        }
        ac.corrected
    }

    /// Apply post-processing + auto-correct to batch results (no blacklist)
    async fn finalize_batch(&self, results: &mut [BatchTranslationResult], from: &str, to: &str) {
        let pp = self.post_processor.lock().await;
        for result in results.iter_mut() {
            result.translated = pp.process(&result.translated);
            let ac = pp.auto_correct(&result.translated, &result.original, from, to);
            if !ac.warnings.is_empty() {
                tracing::warn!(
                    "[Translation] Auto-correct warnings for batch segment {}: {:?}",
                    result.index,
                    ac.warnings
                );
            }
            result.translated = ac.corrected;
        }
    }

    /// Translate text with full pipeline: pre-process -> glossary -> blacklist -> TM -> cache -> engine -> restore -> cache -> history
    pub async fn translate(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslateResponse, TranslationError> {
        tracing::info!("[Translation] Input text ({} chars): {:?}", text.len(), &text[..text.len().min(200)]);

        let prepared = self.prepare(text, from, to).await;

        // Get TM config
        let (tm_enabled, tm_threshold) = {
            let config = self.config.lock().await;
            (config.tm_enabled, config.tm_threshold)
        };

        // Check Translation Memory before cache
        if tm_enabled {
            let history = self.history.lock().await;
            if let Some(tm_match) = history.fuzzy_match(&prepared.text, from, to, tm_threshold) {
                drop(history);
                self.metrics.record_cache_hit().await;
                tracing::info!(
                    "[TM] Hit: similarity={:.2}, engine={}, stored_source={:?}",
                    tm_match.similarity, tm_match.engine, &tm_match.source_text[..tm_match.source_text.len().min(50)]
                );
                let final_text = self.finalize(&tm_match.translated_text, text, from, to, &prepared.blacklist).await;
                return Ok(TranslateResponse {
                    results: vec![TranslationResult {
                        engine: format!("TM ({})", tm_match.engine),
                        text: final_text,
                        latency_ms: None,
                    }],
                    detected_language: None,
                });
            }
        }

        // Check cache first
        if let Some(cached) = self.cache.get(&prepared.text, from, to).await {
            self.metrics.record_cache_hit().await;
            let mut results = Vec::with_capacity(cached.results.len());
            for (engine, cached_text) in cached.results {
                let final_text = self.finalize(&cached_text, text, from, to, &prepared.blacklist).await;
                results.push(TranslationResult {
                    engine,
                    text: final_text,
                    latency_ms: None,
                });
            }
            return Ok(TranslateResponse {
                results,
                detected_language: None,
            });
        }
        self.metrics.record_cache_miss().await;

        // Call translation engines with timing
        let start = Instant::now();
        let router = self.engine_router.read().await;
        let mut response = if prepared.glossary_hint.is_empty() {
            router.translate_all(&prepared.text, from, to).await
        } else {
            let primary_result = router
                .translate_primary_with_glossary(&prepared.text, from, to, &prepared.glossary_hint)
                .await;
            let mut resp = router.translate_rest(&prepared.text, from, to).await;
            match primary_result {
                Ok(translated) => {
                    let engine_name = router.primary_engine_name().unwrap_or("LLM").to_string();
                    resp.results.insert(
                        0,
                        TranslationResult {
                            engine: engine_name,
                            text: translated,
                            latency_ms: None,
                        },
                    );
                }
                Err(e) => {
                    tracing::warn!("[translate] Primary engine with glossary failed: {}, falling back", e);
                    let fallback = router.translate_primary(&prepared.text, from, to).await;
                    if let Ok(translated) = fallback {
                        let engine_name = router.primary_engine_name().unwrap_or("primary").to_string();
                        resp.results.insert(
                            0,
                            TranslationResult {
                                engine: engine_name,
                                text: translated,
                                latency_ms: None,
                            },
                        );
                    }
                }
            }
            resp
        };
        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Record failures for empty results
        let engine_names = router.engine_names();
        drop(router);

        for result in &response.results {
            self.metrics.record_engine_latency(&result.engine, elapsed_ms).await;
        }

        if response.results.is_empty() {
            let detail = if engine_names.is_empty() {
                "No engines are configured".to_string()
            } else {
                format!("No engine returned a result (configured: {})", engine_names.join(", "))
            };
            self.metrics.record_failure("all", &detail).await;
            return Err(TranslationError::AllEnginesFailed {
                errors: vec![detail],
            });
        }

        // Finalize all results: restore blacklist -> post-process -> auto-correct
        for result in &mut response.results {
            result.text = self.finalize(&result.text, text, from, to, &prepared.blacklist).await;
        }

        // Log translation results
        for result in &response.results {
            tracing::info!("[Translation] Engine: {}, Result ({} chars): {:?}", result.engine, result.text.len(), &result.text[..result.text.len().min(200)]);
        }

        // Cache the results
        if !response.results.is_empty() {
            let cache_results: Vec<(String, String)> = response
                .results
                .iter()
                .map(|r| (r.engine.clone(), r.text.clone()))
                .collect();
            self.cache.set(&prepared.text, from, to, cache_results).await;
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
    ) -> Result<String, TranslationError> {
        let prepared = self.prepare(text, from, to).await;

        // Check cache first
        if let Some(cached) = self.cache.get(&prepared.text, from, to).await {
            if let Some((_, cached_text)) = cached.results.into_iter().next() {
                self.metrics.record_cache_hit().await;
                let final_text = self.finalize(&cached_text, text, from, to, &prepared.blacklist).await;
                let _ = tx.send(final_text.clone()).await;
                return Ok(final_text);
            }
        }
        self.metrics.record_cache_miss().await;

        // Stream translation using primary engine
        let start = Instant::now();
        let router = self.engine_router.read().await;
        let result = if prepared.glossary_hint.is_empty() {
            router.translate_stream(&prepared.text, from, to, tx).await
        } else {
            router.translate_stream_with_glossary(&prepared.text, from, to, tx, &prepared.glossary_hint).await
        };
        drop(router);

        match result {
            Ok(full_text) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                self.metrics.record_engine_latency("LLM", elapsed_ms).await;

                let final_text = self.finalize(&full_text, text, from, to, &prepared.blacklist).await;

                if !final_text.is_empty() {
                    self.cache
                        .set(&prepared.text, from, to, vec![("LLM".to_string(), final_text.clone())])
                        .await;
                    let history = self.history.lock().await;
                    history.add(text, &final_text, from, to, "LLM");
                }

                Ok(final_text)
            }
            Err(e) => {
                self.metrics.record_failure("LLM", &e.to_string()).await;
                Err(TranslationError::EngineError {
                    engine: "LLM".to_string(),
                    message: format!("Streaming failed: {}", e),
                })
            }
        }
    }

    /// Translate with primary engine only (for quick translations)
    pub async fn translate_primary(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, TranslationError> {
        let prepared = self.prepare(text, from, to).await;

        // Check Translation Memory
        let (tm_enabled, tm_threshold) = {
            let config = self.config.lock().await;
            (config.tm_enabled, config.tm_threshold)
        };

        if tm_enabled {
            let history = self.history.lock().await;
            if let Some(tm_match) = history.fuzzy_match(&prepared.text, from, to, tm_threshold) {
                drop(history);
                self.metrics.record_cache_hit().await;
                let final_text = self.finalize(&tm_match.translated_text, text, from, to, &prepared.blacklist).await;
                return Ok(final_text);
            }
        }

        let start = Instant::now();
        let router = self.engine_router.read().await;
        let result = if prepared.glossary_hint.is_empty() {
            router.translate_primary(&prepared.text, from, to).await
        } else {
            router.translate_primary_with_glossary(&prepared.text, from, to, &prepared.glossary_hint).await
        };
        drop(router);

        match result {
            Ok(translated) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                self.metrics.record_engine_latency("primary", elapsed_ms).await;

                let final_text = self.finalize(&translated, text, from, to, &prepared.blacklist).await;
                Ok(final_text)
            }
            Err(e) => {
                self.metrics.record_failure("primary", &e.to_string()).await;
                Err(TranslationError::from(e))
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
    ) -> Result<String, TranslationError> {
        let router = self.engine_router.read().await;
        router
            .translate_primary_with_context(text, from, to, context)
            .await
            .map_err(TranslationError::from)
    }

    /// Get the engine router for advanced operations
    pub fn router(&self) -> &Arc<RwLock<Router>> {
        &self.engine_router
    }

    /// Batch translate multiple lines with concurrency control and context reuse
    /// Returns results in the same order as input
    pub async fn translate_batch(
        &self,
        lines: &[(usize, &str)],
        from: &str,
        to: &str,
        concurrency: usize,
    ) -> Vec<BatchTranslationResult> {
        if lines.is_empty() {
            return Vec::new();
        }

        let concurrency = concurrency.max(1).min(10);
        let mut results = Vec::with_capacity(lines.len());
        let mut context: Vec<TranslationContext> = Vec::new();

        self.metrics.record_chunk_size(lines.len()).await;

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
                    let translated = match router
                        .translate_primary_with_context(&text, &from, &to, &context_snapshot)
                        .await
                    {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::warn!("[translate_batch] Translation failed for segment {}: {}", idx, e);
                            String::new()
                        }
                    };
                    drop(router);

                    BatchTranslationResult {
                        index: idx,
                        original: text,
                        translated,
                    }
                });

                handles.push(handle);
            }

            for handle in handles {
                if let Ok(result) = handle.await {
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

        results.sort_by_key(|r| r.index);
        self.finalize_batch(&mut results, from, to).await;
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

        self.metrics.record_chunk_size(total).await;

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
                    let translated = match router
                        .translate_primary_with_context(&text, &from, &to, &context_snapshot)
                        .await
                    {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::warn!("[translate_batch] Translation failed for segment {}: {}", idx, e);
                            String::new()
                        }
                    };
                    drop(router);

                    BatchTranslationResult {
                        index: idx,
                        original: text,
                        translated,
                    }
                });

                handles.push(handle);
            }

            for handle in handles {
                if let Ok(result) = handle.await {
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

        results.sort_by_key(|r| r.index);
        self.finalize_batch(&mut results, from, to).await;
        results
    }
}

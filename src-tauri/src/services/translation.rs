use crate::blacklist::BlacklistProcessor;
use crate::cache::TranslationCache;
use crate::config::AppConfig;
use crate::engine::{llm::TranslationContext, Router, TranslateResponse, TranslationResult};
use crate::glossary::Glossary;
use crate::memory::HistoryStore;
use crate::metrics::MetricsCollector;
use crate::models::error::TranslationError;
use crate::pre_process::PreProcessor;
pub use crate::models::translation::BatchTranslationResult;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

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
    ) -> Self {
        Self {
            config,
            glossary,
            history,
            cache,
            engine_router,
            metrics,
            pre_processor,
        }
    }

    /// Translate text with full pipeline: pre-process -> glossary -> blacklist -> TM -> cache -> engine -> restore -> cache -> history
    pub async fn translate(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslateResponse, TranslationError> {
        log::info!("[Translation] Input text ({} chars): {:?}", text.len(), &text[..text.len().min(200)]);

        // Apply pre-processing (regex rules, unicode normalization, etc.)
        let lang_pair = format!("{}-{}", from, to);
        let mut processed_text = self.pre_processor.process(text, Some(&lang_pair));

        // Apply glossary
        let glossary = self.glossary.lock().await;
        glossary.apply_glossary(&mut processed_text, &lang_pair);
        let glossary_hint = glossary.format_hint(&lang_pair);
        drop(glossary);

        // Apply blacklist protection
        let config = self.config.lock().await;
        let blacklist_processor = BlacklistProcessor::new(config.translation_blacklist.clone());
        let tm_enabled = config.tm_enabled;
        let tm_threshold = config.tm_threshold;
        drop(config);

        let (protected_text, placeholder_map) = blacklist_processor.protect(&processed_text);
        let has_blacklist = !placeholder_map.is_empty();

        // Check Translation Memory before cache
        if tm_enabled {
            let history = self.history.lock().await;
            if let Some(tm_match) = history.fuzzy_match(&protected_text, from, to, tm_threshold) {
                drop(history);
                self.metrics.record_cache_hit().await; // Reuse cache hit metric for TM
                log::info!(
                    "[TM] Hit: similarity={:.2}, engine={}, stored_source={:?}",
                    tm_match.similarity, tm_match.engine, &tm_match.source_text[..tm_match.source_text.len().min(50)]
                );
                let final_text = if has_blacklist {
                    blacklist_processor.restore(&tm_match.translated_text, &placeholder_map)
                } else {
                    tm_match.translated_text
                };
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
                        latency_ms: None,
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
        let mut response = if glossary_hint.is_empty() {
            router.translate_all(&protected_text, from, to).await
        } else {
            // Primary engine gets glossary hint injected into system prompt;
            // other engines use standard text-replacement glossary
            let primary_result = router
                .translate_primary_with_glossary(&protected_text, from, to, &glossary_hint)
                .await;
            let mut resp = router.translate_rest(&protected_text, from, to).await;
            match primary_result {
                Ok(text) => {
                    let engine_name = router.primary_engine_name().unwrap_or("LLM").to_string();
                    resp.results.insert(
                        0,
                        TranslationResult {
                            engine: engine_name,
                            text,
                            latency_ms: None,
                        },
                    );
                }
                Err(e) => {
                    log::warn!("[translate] Primary engine with glossary failed: {}, falling back", e);
                    let fallback = router.translate_primary(&protected_text, from, to).await;
                    if let Ok(text) = fallback {
                        let engine_name = router.primary_engine_name().unwrap_or("primary").to_string();
                        resp.results.insert(
                            0,
                            TranslationResult {
                                engine: engine_name,
                                text,
                                latency_ms: None,
                            },
                        );
                    }
                }
            }
            resp
        };
        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Record failures for empty results (check before dropping router)
        let engine_names = router.engine_names();
        drop(router);

        // Record engine latency for each result
        for result in &response.results {
            self.metrics
                .record_engine_latency(&result.engine, elapsed_ms)
                .await;
        }

        if response.results.is_empty() {
            let detail = if engine_names.is_empty() {
                "No engines are configured".to_string()
            } else {
                format!(
                    "No engine returned a result (configured: {})",
                    engine_names.join(", ")
                )
            };
            self.metrics
                .record_failure("all", &detail)
                .await;
            return Err(TranslationError::AllEnginesFailed {
                errors: vec![detail],
            });
        }

        // Restore blacklist words in results
        if has_blacklist {
            for result in &mut response.results {
                result.text = blacklist_processor.restore(&result.text, &placeholder_map);
            }
        }

        // Log translation results
        for result in &response.results {
            log::info!("[Translation] Engine: {}, Result ({} chars): {:?}", result.engine, result.text.len(), &result.text[..result.text.len().min(200)]);
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
    ) -> Result<String, TranslationError> {
        // Apply pre-processing
        let lang_pair = format!("{}-{}", from, to);
        let mut processed_text = self.pre_processor.process(text, Some(&lang_pair));

        // Apply glossary
        let glossary = self.glossary.lock().await;
        glossary.apply_glossary(&mut processed_text, &lang_pair);
        let glossary_hint = glossary.format_hint(&lang_pair);
        drop(glossary);

        // Apply blacklist protection
        let config = self.config.lock().await;
        let blacklist_processor = BlacklistProcessor::new(config.translation_blacklist.clone());
        drop(config);

        let (protected_text, placeholder_map) = blacklist_processor.protect(&processed_text);
        let has_blacklist = !placeholder_map.is_empty();

        // Check cache first
        if let Some(cached) = self.cache.get(&protected_text, from, to).await {
            if let Some((_, cached_text)) = cached.results.first() {
                self.metrics.record_cache_hit().await;
                let final_text = if has_blacklist {
                    blacklist_processor.restore(cached_text, &placeholder_map)
                } else {
                    cached_text.clone()
                };
                let _ = tx.send(final_text.clone()).await;
                return Ok(final_text);
            }
        }
        self.metrics.record_cache_miss().await;

        // Stream translation using primary engine
        let start = Instant::now();
        let router = self.engine_router.read().await;
        let result = if glossary_hint.is_empty() {
            router.translate_stream(&protected_text, from, to, tx).await
        } else {
            router.translate_stream_with_glossary(&protected_text, from, to, tx, &glossary_hint).await
        };
        drop(router);

        match result {
            Ok(full_text) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                self.metrics.record_engine_latency("LLM", elapsed_ms).await;

                // Restore blacklist words
                let final_text = if has_blacklist {
                    blacklist_processor.restore(&full_text, &placeholder_map)
                } else {
                    full_text
                };

                // Cache the result (with blacklist protection applied)
                if !final_text.is_empty() {
                    self.cache
                        .set(
                            &protected_text,
                            from,
                            to,
                            vec![("LLM".to_string(), final_text.clone())],
                        )
                        .await;

                    // Save to history
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
        // Apply pre-processing
        let lang_pair = format!("{}-{}", from, to);
        let mut processed_text = self.pre_processor.process(text, Some(&lang_pair));

        // Apply glossary
        let glossary = self.glossary.lock().await;
        glossary.apply_glossary(&mut processed_text, &lang_pair);
        let glossary_hint = glossary.format_hint(&lang_pair);
        drop(glossary);

        // Apply blacklist protection
        let config = self.config.lock().await;
        let blacklist_processor = BlacklistProcessor::new(config.translation_blacklist.clone());
        let tm_enabled = config.tm_enabled;
        let tm_threshold = config.tm_threshold;
        drop(config);

        let (protected_text, placeholder_map) = blacklist_processor.protect(&processed_text);
        let has_blacklist = !placeholder_map.is_empty();

        // Check Translation Memory
        if tm_enabled {
            let history = self.history.lock().await;
            if let Some(tm_match) = history.fuzzy_match(&protected_text, from, to, tm_threshold) {
                drop(history);
                self.metrics.record_cache_hit().await;
                let final_text = if has_blacklist {
                    blacklist_processor.restore(&tm_match.translated_text, &placeholder_map)
                } else {
                    tm_match.translated_text
                };
                return Ok(final_text);
            }
        }

        let start = Instant::now();

        let router = self.engine_router.read().await;
        let result = if glossary_hint.is_empty() {
            router.translate_primary(&protected_text, from, to).await
        } else {
            router.translate_primary_with_glossary(&protected_text, from, to, &glossary_hint).await
        };
        drop(router);

        match result {
            Ok(translated) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                self.metrics
                    .record_engine_latency("primary", elapsed_ms)
                    .await;

                // Restore blacklist words
                let final_text = if has_blacklist {
                    blacklist_processor.restore(&translated, &placeholder_map)
                } else {
                    translated
                };
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

        // Record batch chunk size
        self.metrics.record_chunk_size(lines.len()).await;

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
                    let translated = match router
                        .translate_primary_with_context(&text, &from, &to, &context_snapshot)
                        .await
                    {
                        Ok(t) => t,
                        Err(e) => {
                            log::warn!("[translate_batch] Translation failed for segment {}: {}", idx, e);
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

        // Record batch chunk size
        self.metrics.record_chunk_size(total).await;

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
                    let translated = match router
                        .translate_primary_with_context(&text, &from, &to, &context_snapshot)
                        .await
                    {
                        Ok(t) => t,
                        Err(e) => {
                            log::warn!("[translate_batch] Translation failed for segment {}: {}", idx, e);
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

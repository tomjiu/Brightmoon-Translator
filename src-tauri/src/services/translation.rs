use crate::blacklist::BlacklistProcessor;
use crate::cache::TranslationCache;
use crate::config::AppConfig;
use crate::engine::{llm::TranslationContext, Router, TranslateResponse, TranslationResult};
use crate::glossary::Glossary;
use crate::memory::HistoryStore;
use crate::metrics::MetricsCollector;
use crate::models::error::TranslationError;
pub use crate::models::translation::BatchTranslationResult;
use crate::models::translation::{
    RoutingStrategy, TranslateChannel, TranslateOutcome, TranslateRequest, TranslationMode,
};
use crate::post_process::PostProcessor;
use crate::pre_process::PreProcessor;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};
use tracing::{info_span, Instrument};

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
        let processed = pp.process_with_source(&restored, Some(source));
        let ac = pp.auto_correct(&processed, source, from, to);
        if !ac.warnings.is_empty() {
            tracing::warn!("[Translation] Auto-correct warnings: {:?}", ac.warnings);
        }
        ac.corrected
    }

    /// Apply post-processing + auto-correct to batch results (no blacklist).
    /// Prefer per-segment `finalize` in batch core; kept for callers that skip prepare.
    #[allow(dead_code)]
    async fn finalize_batch(&self, results: &mut [BatchTranslationResult], from: &str, to: &str) {
        let pp = self.post_processor.lock().await;
        for result in results.iter_mut() {
            result.translated = pp.process_with_source(&result.translated, Some(&result.original));
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

    /// Façade entry: dispatch by `TranslateRequest.mode` (stream uses `translate_stream`).
    pub async fn run(&self, req: TranslateRequest) -> Result<TranslateOutcome, TranslationError> {
        let channel = format!("{:?}", req.channel);
        let span = info_span!(
            "translate_run",
            channel = %channel,
            mode = ?req.mode,
            chars = req.text.len(),
            from = %req.from,
            to = %req.to,
        );
        async {
            match req.mode {
                TranslationMode::Full => {
                    let r = self
                        .translate(req.channel, &req.text, &req.from, &req.to)
                        .await?;
                    Ok(TranslateOutcome::Full(r))
                },
                TranslationMode::Primary => {
                    let r = self
                        .translate_primary(&req.text, &req.from, &req.to)
                        .await?;
                    Ok(TranslateOutcome::Primary(r))
                },
                TranslationMode::Batch => {
                    let lines: Vec<(usize, &str)> = if req.segments.is_empty() {
                        req.text
                            .lines()
                            .enumerate()
                            .filter(|(_, l)| !l.trim().is_empty())
                            .map(|(i, l)| (i, l.trim()))
                            .collect()
                    } else {
                        req.segments.iter().map(|(i, s)| (*i, s.as_str())).collect()
                    };
                    let r = self
                        .translate_batch(req.channel, &lines, &req.from, &req.to, req.concurrency)
                        .await;
                    Ok(TranslateOutcome::Batch(r))
                },
                TranslationMode::Context => {
                    let ctx: Vec<TranslationContext> = req
                        .context
                        .iter()
                        .map(|c| TranslationContext {
                            source: c.source.clone(),
                            translation: c.translation.clone(),
                        })
                        .collect();
                    let r = self
                        .translate_with_context(&req.text, &req.from, &req.to, &ctx)
                        .await?;
                    Ok(TranslateOutcome::Primary(r))
                },
                TranslationMode::Compare => {
                    let router = self.engine_router.read().await;
                    let response = router
                        .translate_parallel_compare(&req.text, &req.from, &req.to)
                        .await;
                    drop(router);
                    Ok(TranslateOutcome::Full(response))
                },
                TranslationMode::Stream => Err(TranslationError::StreamingNotSupported),
            }
        }
        .instrument(span)
        .await
    }

    /// Convenience: full-mode run for a product channel.
    pub async fn run_full(
        &self,
        channel: TranslateChannel,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslateResponse, TranslationError> {
        match self
            .run(TranslateRequest::full(channel, text, from, to))
            .await?
        {
            TranslateOutcome::Full(r) => Ok(r),
            other => Err(TranslationError::Internal(format!(
                "unexpected outcome for full: {:?}",
                other
            ))),
        }
    }

    /// Convenience: primary-mode run for a product channel.
    pub async fn run_primary(
        &self,
        channel: TranslateChannel,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, TranslationError> {
        match self
            .run(TranslateRequest::primary(channel, text, from, to))
            .await?
        {
            TranslateOutcome::Primary(s) => Ok(s),
            other => Err(TranslationError::Internal(format!(
                "unexpected outcome for primary: {:?}",
                other
            ))),
        }
    }

    /// Convenience: batch-mode run (segments as index+text pairs).
    pub async fn run_batch(
        &self,
        channel: TranslateChannel,
        lines: &[(usize, &str)],
        from: &str,
        to: &str,
        concurrency: usize,
    ) -> Vec<BatchTranslationResult> {
        let segments: Vec<(usize, String)> =
            lines.iter().map(|(i, s)| (*i, (*s).to_string())).collect();
        match self
            .run(TranslateRequest {
                channel,
                mode: TranslationMode::Batch,
                text: String::new(),
                from: from.to_string(),
                to: to.to_string(),
                concurrency: concurrency.max(1).min(10),
                segments,
                ..Default::default()
            })
            .await
        {
            Ok(TranslateOutcome::Batch(b)) => b,
            Ok(other) => {
                tracing::warn!("[run_batch] unexpected outcome: {:?}", other);
                Vec::new()
            },
            Err(e) => {
                tracing::warn!("[run_batch] failed: {}", e);
                Vec::new()
            },
        }
    }

    /// Product-channel default routing (overrides global `routingStrategy` for UI / OCR).
    fn strategy_for_channel(
        channel: TranslateChannel,
        configured: RoutingStrategy,
    ) -> RoutingStrategy {
        match channel {
            // Main window: always multi-engine results
            TranslateChannel::Ui => RoutingStrategy::ParallelCompare,
            // OCR frame: single result with ordered fallback (do not change)
            TranslateChannel::Ocr => RoutingStrategy::FallbackOnError,
            // Selection / hover popups: use global strategy (user-configurable in settings)
            // so划词 can opt into ParallelCompare without hard-coding here.
            TranslateChannel::Selection
            | TranslateChannel::Clipboard
            | TranslateChannel::Replace => configured,
            _ => configured,
        }
    }

    /// Translate text with full pipeline: pre-process -> glossary -> blacklist -> TM -> cache -> engine -> restore -> cache -> history
    pub async fn translate(
        &self,
        channel: TranslateChannel,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslateResponse, TranslationError> {
        let span = info_span!(
            "translate",
            channel = ?channel,
            chars = text.len(),
            from,
            to
        );
        async {
            let preview: String = text.chars().take(100).collect();
            tracing::info!(
                "[Translation] Input text ({} chars): {:?}",
                text.len(),
                preview
            );

            let prepared = self.prepare(text, from, to).await;

            let strategy = {
                let config = self.config.lock().await;
                Self::strategy_for_channel(
                    channel,
                    config.routing_strategy.clone().unwrap_or_default(),
                )
            };
            let want_multi = matches!(strategy, RoutingStrategy::ParallelCompare);

            // Get TM config
            let (tm_enabled, tm_threshold) = {
                let config = self.config.lock().await;
                (config.tm_enabled, config.tm_threshold)
            };

            // TM is single-result; skip for multi-result UI so homepage still compares engines
            if tm_enabled && !want_multi {
                let history = self.history.lock().await;
                if let Some(tm_match) = history.fuzzy_match(&prepared.text, from, to, tm_threshold)
                {
                    drop(history);
                    self.metrics.record_cache_hit();
                    let tm_preview: String = tm_match.source_text.chars().take(50).collect();
                    tracing::info!(
                        "[TM] Hit: similarity={:.2}, engine={}, stored_source={:?}",
                        tm_match.similarity,
                        tm_match.engine,
                        tm_preview
                    );
                    let final_text = self
                        .finalize(
                            &tm_match.translated_text,
                            text,
                            from,
                            to,
                            &prepared.blacklist,
                        )
                        .await;
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

            // Check cache first (UI multi needs ≥2 engines; single-entry cache is a miss)
            let cache_span = info_span!("cache_lookup");
            let cached = async { self.cache.get(&prepared.text, from, to).await }
                .instrument(cache_span)
                .await;
            if let Some(cached) = cached {
                let usable = !want_multi || cached.results.len() >= 2;
                if usable {
                    self.metrics.record_cache_hit();
                    let mut results = Vec::with_capacity(cached.results.len());
                    for (engine, cached_text) in cached.results {
                        let final_text = self
                            .finalize(&cached_text, text, from, to, &prepared.blacklist)
                            .await;
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
            }
            self.metrics.record_cache_miss();

            // Call translation engines with timing
            let start = Instant::now();
            let router = self.engine_router.read().await;
            let mut response = if prepared.glossary_hint.is_empty() {
                let engine_span = info_span!("engine_translate_strategy", ?strategy);
                async {
                    router
                        .translate_with_strategy(strategy.clone(), &prepared.text, from, to)
                        .await
                }
                .instrument(engine_span)
                .await
            } else if want_multi {
                // Glossary on primary + parallel rest (main window multi-result)
                let engine_span = info_span!("engine_translate_glossary_multi");
                async {
                    let primary_result = router
                        .translate_primary_with_glossary(
                            &prepared.text,
                            from,
                            to,
                            &prepared.glossary_hint,
                        )
                        .await;
                    let mut resp = router.translate_rest(&prepared.text, from, to).await;
                    match primary_result {
                        Ok(translated) => {
                            let engine_name =
                                router.primary_engine_name().unwrap_or("LLM").to_string();
                            resp.results.insert(
                                0,
                                TranslationResult {
                                    engine: engine_name,
                                    text: translated,
                                    latency_ms: None,
                                },
                            );
                        },
                        Err(e) => {
                            tracing::warn!(
                                "[translate] Primary engine with glossary failed: {}, falling back",
                                e
                            );
                            let fallback = router
                                .translate_fallback_string(&prepared.text, from, to)
                                .await;
                            if let Ok(translated) = fallback {
                                let engine_name = router
                                    .primary_engine_name()
                                    .unwrap_or("primary")
                                    .to_string();
                                resp.results.insert(
                                    0,
                                    TranslationResult {
                                        engine: engine_name,
                                        text: translated,
                                        latency_ms: None,
                                    },
                                );
                            }
                        },
                    }
                    resp
                }
                .instrument(engine_span)
                .await
            } else {
                // Single-result path (OCR / fallback): glossary primary, then ordered fallback
                let engine_span = info_span!("engine_translate_glossary_single");
                async {
                    match router
                        .translate_primary_with_glossary(
                            &prepared.text,
                            from,
                            to,
                            &prepared.glossary_hint,
                        )
                        .await
                    {
                        Ok(translated) => TranslateResponse {
                            results: vec![TranslationResult {
                                engine: router.primary_engine_name().unwrap_or("LLM").to_string(),
                                text: translated,
                                latency_ms: None,
                            }],
                            detected_language: None,
                        },
                        Err(e) => {
                            tracing::warn!(
                                "[translate] Primary with glossary failed: {}, ordered fallback",
                                e
                            );
                            router
                                .translate_with_strategy(
                                    RoutingStrategy::FallbackOnError,
                                    &prepared.text,
                                    from,
                                    to,
                                )
                                .await
                        },
                    }
                }
                .instrument(engine_span)
                .await
            };
            let elapsed_ms = start.elapsed().as_millis() as u64;

            // Record failures for empty results
            let engine_names: Vec<String> = router
                .engine_names()
                .iter()
                .map(|s| s.to_string())
                .collect();
            drop(router);

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
                self.metrics.record_failure("all", &detail).await;
                return Err(TranslationError::AllEnginesFailed {
                    errors: vec![detail],
                });
            }

            // Finalize all results: restore blacklist -> post-process -> auto-correct
            for result in &mut response.results {
                result.text = self
                    .finalize(&result.text, text, from, to, &prepared.blacklist)
                    .await;
            }

            // Log translation results (char-safe prefix — never index UTF-8 by raw bytes)
            for result in &response.results {
                let preview: String = result.text.chars().take(80).collect();
                tracing::info!(
                    "[Translation] Engine: {}, Result ({} chars): {:?}",
                    result.engine,
                    result.text.chars().count(),
                    preview
                );
            }

            // Cache the results
            if !response.results.is_empty() {
                let cache_results: Vec<(String, String)> = response
                    .results
                    .iter()
                    .map(|r| (r.engine.clone(), r.text.clone()))
                    .collect();
                self.cache
                    .set(&prepared.text, from, to, cache_results)
                    .await;
            }

            // Save to history
            if let Some(first) = response.results.first() {
                let history = self.history.lock().await;
                history.add(text, &first.text, from, to, &first.engine);
            }

            Ok(response)
        }
        .instrument(span)
        .await
    }

    /// Stream translation using primary engine
    pub async fn translate_stream(
        &self,
        text: &str,
        from: &str,
        to: &str,
        tx: tokio::sync::mpsc::Sender<String>,
    ) -> Result<String, TranslationError> {
        let span = info_span!("translate_stream", chars = text.len(), from, to);
        async {
            let prepared = self.prepare(text, from, to).await;

            // Check cache first
            let cache_span = info_span!("cache_lookup");
            let cached = async { self.cache.get(&prepared.text, from, to).await }
                .instrument(cache_span)
                .await;
            if let Some(cached) = cached {
                if let Some((_, cached_text)) = cached.results.into_iter().next() {
                    self.metrics.record_cache_hit();
                    let final_text = self
                        .finalize(&cached_text, text, from, to, &prepared.blacklist)
                        .await;
                    let _ = tx.send(final_text.clone()).await;
                    return Ok(final_text);
                }
            }
            self.metrics.record_cache_miss();

            // Stream translation using primary engine
            let start = Instant::now();
            let router = self.engine_router.read().await;
            let engine_span = info_span!("engine_stream");
            let result = if prepared.glossary_hint.is_empty() {
                async { router.translate_stream(&prepared.text, from, to, tx).await }
                    .instrument(engine_span)
                    .await
            } else {
                async {
                    router
                        .translate_stream_with_glossary(
                            &prepared.text,
                            from,
                            to,
                            tx,
                            &prepared.glossary_hint,
                        )
                        .await
                }
                .instrument(engine_span)
                .await
            };
            drop(router);

            match result {
                Ok(full_text) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    self.metrics.record_engine_latency("LLM", elapsed_ms).await;

                    let final_text = self
                        .finalize(&full_text, text, from, to, &prepared.blacklist)
                        .await;

                    if !final_text.is_empty() {
                        self.cache
                            .set(
                                &prepared.text,
                                from,
                                to,
                                vec![("LLM".to_string(), final_text.clone())],
                            )
                            .await;
                        let history = self.history.lock().await;
                        history.add(text, &final_text, from, to, "LLM");
                    }

                    Ok(final_text)
                },
                Err(e) => {
                    self.metrics.record_failure("LLM", &e.to_string()).await;
                    Err(TranslationError::EngineError {
                        engine: "LLM".to_string(),
                        message: format!("Streaming failed: {}", e),
                    })
                },
            }
        }
        .instrument(span)
        .await
    }

    /// Translate with primary engine only (for quick translations)
    pub async fn translate_primary(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, TranslationError> {
        let span = info_span!("translate_primary", chars = text.len(), from, to);
        async {
            let prepared = self.prepare(text, from, to).await;

            // Check Translation Memory
            let (tm_enabled, tm_threshold) = {
                let config = self.config.lock().await;
                (config.tm_enabled, config.tm_threshold)
            };

            if tm_enabled {
                let history = self.history.lock().await;
                if let Some(tm_match) = history.fuzzy_match(&prepared.text, from, to, tm_threshold)
                {
                    drop(history);
                    self.metrics.record_cache_hit();
                    let final_text = self
                        .finalize(
                            &tm_match.translated_text,
                            text,
                            from,
                            to,
                            &prepared.blacklist,
                        )
                        .await;
                    return Ok(final_text);
                }
            }

            let start = Instant::now();
            let router = self.engine_router.read().await;
            let engine_span = info_span!("engine_call");
            let result = if prepared.glossary_hint.is_empty() {
                async { router.translate_primary(&prepared.text, from, to).await }
                    .instrument(engine_span)
                    .await
            } else {
                async {
                    router
                        .translate_primary_with_glossary(
                            &prepared.text,
                            from,
                            to,
                            &prepared.glossary_hint,
                        )
                        .await
                }
                .instrument(engine_span)
                .await
            };
            drop(router);

            match result {
                Ok(translated) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    self.metrics
                        .record_engine_latency("primary", elapsed_ms)
                        .await;

                    let final_text = self
                        .finalize(&translated, text, from, to, &prepared.blacklist)
                        .await;
                    Ok(final_text)
                },
                Err(e) => {
                    self.metrics.record_failure("primary", &e.to_string()).await;
                    Err(TranslationError::from(e))
                },
            }
        }
        .instrument(span)
        .await
    }

    /// Translate with context for document consistency (prepare + finalize)
    pub async fn translate_with_context(
        &self,
        text: &str,
        from: &str,
        to: &str,
        context: &[crate::engine::llm::TranslationContext],
    ) -> Result<String, TranslationError> {
        let prepared = self.prepare(text, from, to).await;
        let router = self.engine_router.read().await;
        let raw = router
            .translate_primary_with_context(&prepared.text, from, to, context)
            .await
            .map_err(TranslationError::from)?;
        drop(router);
        Ok(self
            .finalize(&raw, text, from, to, &prepared.blacklist)
            .await)
    }

    /// Get the engine router for advanced operations
    pub fn router(&self) -> &Arc<RwLock<Router>> {
        &self.engine_router
    }

    /// Core batch translation logic shared by translate_batch and translate_embedded_batch.
    /// Spawns concurrent translation tasks with context reuse and optional progress callback.
    /// When primary is LLM and multi-seg, uses numbered pack/parse (A4) instead of N calls.
    /// OCR channel uses ordered engine fallback per segment (or after LLM pack failure).
    async fn translate_batch_core<F>(
        &self,
        channel: TranslateChannel,
        lines: &[(usize, &str)],
        from: &str,
        to: &str,
        concurrency: usize,
        mut on_progress: F,
    ) -> Vec<BatchTranslationResult>
    where
        F: FnMut(usize, usize),
    {
        let ocr_fallback = matches!(channel, TranslateChannel::Ocr);
        let span = info_span!(
            "translate_batch",
            channel = ?channel,
            total = lines.len(),
            from,
            to
        );
        async {
            let total = lines.len();
            if total == 0 {
                return Vec::new();
            }

            let concurrency = concurrency.max(1).min(10);
            let mut results = Vec::with_capacity(total);
            // Use VecDeque for O(1) pop_front when evicting old context entries
            let mut context: VecDeque<TranslationContext> = VecDeque::new();
            let mut completed = 0;

            self.metrics.record_chunk_size(total).await;

            let use_llm_numbered = {
                let router = self.engine_router.read().await;
                router.primary_is_llm() && total > 1
            };

            if use_llm_numbered {
                // Prepare all segments, then pack numbered LLM calls in concurrency-sized chunks
                let mut prepared_rows: Vec<(usize, String, PreparedText)> =
                    Vec::with_capacity(total);
                for &(idx, text) in lines {
                    let prepared = self.prepare(text, from, to).await;
                    prepared_rows.push((idx, text.to_string(), prepared));
                }

                for chunk in prepared_rows.chunks(concurrency) {
                    let segs: Vec<&str> = chunk.iter().map(|(_, _, p)| p.text.as_str()).collect();
                    let raws = {
                        let router = self.engine_router.read().await;
                        match router
                            .translate_primary_batch_segments(&segs, from, to)
                            .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::warn!(
                                    "[translate_batch] LLM numbered batch failed: {}",
                                    e
                                );
                                if ocr_fallback {
                                    // Per-seg ordered fallback when LLM pack fails
                                    let mut fallbacks = Vec::with_capacity(segs.len());
                                    for seg in &segs {
                                        match router
                                            .translate_fallback_string(seg, from, to)
                                            .await
                                        {
                                            Ok(t) => fallbacks.push(t),
                                            Err(_) => fallbacks.push(String::new()),
                                        }
                                    }
                                    fallbacks
                                } else {
                                    vec![String::new(); segs.len()]
                                }
                            },
                        }
                    };

                    for ((idx, original, prepared), raw) in chunk.iter().zip(raws.into_iter()) {
                        let translated = self
                            .finalize(&raw, original, from, to, &prepared.blacklist)
                            .await;
                        context.push_back(TranslationContext {
                            source: original.clone(),
                            translation: translated.clone(),
                        });
                        while context.len() > 5 {
                            context.pop_front();
                        }
                        results.push(BatchTranslationResult {
                            index: *idx,
                            original: original.clone(),
                            translated,
                        });
                        completed += 1;
                        on_progress(completed, total);
                    }
                }
            } else {
                for chunk in lines.chunks(concurrency) {
                    let mut handles = Vec::new();

                    for &(idx, text) in chunk {
                        // Pipeline parity: pre-process / glossary / blacklist before engine
                        let prepared = self.prepare(text, from, to).await;
                        let original = text.to_string();
                        let protected = prepared.text.clone();
                        let from_s = from.to_string();
                        let to_s = to.to_string();
                        let context_snapshot: Vec<TranslationContext> =
                            context.iter().cloned().collect();
                        let router = self.engine_router.clone();
                        let use_fallback = ocr_fallback;

                        let handle = tokio::spawn(async move {
                            let router = router.read().await;
                            let translated = if use_fallback {
                                match router
                                    .translate_fallback_string(&protected, &from_s, &to_s)
                                    .await
                                {
                                    Ok(t) => t,
                                    Err(e) => {
                                        tracing::warn!(
                                            "[translate_batch] OCR fallback failed for segment {}: {}",
                                            idx,
                                            e
                                        );
                                        String::new()
                                    },
                                }
                            } else {
                                match router
                                    .translate_primary_with_context(
                                        &protected,
                                        &from_s,
                                        &to_s,
                                        &context_snapshot,
                                    )
                                    .await
                                {
                                    Ok(t) => t,
                                    Err(e) => {
                                        tracing::warn!(
                                            "[translate_batch] Translation failed for segment {}: {}",
                                            idx,
                                            e
                                        );
                                        String::new()
                                    },
                                }
                            };
                            drop(router);

                            (idx, original, translated, prepared.blacklist)
                        });

                        handles.push(handle);
                    }

                    for handle in handles {
                        if let Ok((idx, original, raw, blacklist)) = handle.await {
                            let translated =
                                self.finalize(&raw, &original, from, to, &blacklist).await;
                            context.push_back(TranslationContext {
                                source: original.clone(),
                                translation: translated.clone(),
                            });
                            while context.len() > 5 {
                                context.pop_front();
                            }
                            results.push(BatchTranslationResult {
                                index: idx,
                                original,
                                translated,
                            });
                            completed += 1;
                            on_progress(completed, total);
                        }
                    }
                }
            }

            results.sort_by_key(|r| r.index);

            // AiNiee-style segment validation (warn only — do not drop results)
            let response_check_enabled = {
                let pp = self.post_processor.lock().await;
                pp.get_config().response_check
            };
            if response_check_enabled {
                let sources: Vec<String> = results.iter().map(|r| r.original.clone()).collect();
                let translations: Vec<String> =
                    results.iter().map(|r| r.translated.clone()).collect();
                let check = crate::response_check::check_segments(
                    &sources,
                    &translations,
                    &crate::response_check::ResponseCheckOptions::strict(),
                );
                if !check.ok {
                    tracing::warn!("[translate_batch] response check: {}", check.message);
                }
            }

            results
        }
        .instrument(span)
        .await
    }

    /// Batch translate multiple lines with concurrency control and context reuse
    /// Returns results in the same order as input
    pub async fn translate_batch(
        &self,
        channel: TranslateChannel,
        lines: &[(usize, &str)],
        from: &str,
        to: &str,
        concurrency: usize,
    ) -> Vec<BatchTranslationResult> {
        self.translate_batch_core(
            channel,
            lines,
            from,
            to,
            concurrency,
            |_completed, _total| {},
        )
        .await
    }

    /// Translate text lines for embedded/subtitle with progress callback
    pub async fn translate_embedded_batch<F>(
        &self,
        channel: TranslateChannel,
        text: &str,
        from: &str,
        to: &str,
        concurrency: usize,
        on_progress: F,
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

        self.translate_batch_core(channel, &lines, from, to, concurrency, on_progress)
            .await
    }
}

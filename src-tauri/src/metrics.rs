use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::Utc;

/// Translation metrics collector
pub struct MetricsCollector {
    inner: Arc<Mutex<MetricsInner>>,
}

struct MetricsInner {
    /// Engine translation latency (engine_name -> durations in ms)
    engine_latencies: HashMap<String, Vec<u64>>,
    /// Cache hit/miss counts
    cache_hits: u64,
    cache_misses: u64,
    /// OCR latency
    ocr_latencies: Vec<u64>,
    /// Translation failures by engine
    failures: HashMap<String, Vec<FailureRecord>>,
    /// Document chunk sizes
    chunk_sizes: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct FailureRecord {
    error: String,
    timestamp: i64,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MetricsInner {
                engine_latencies: HashMap::new(),
                cache_hits: 0,
                cache_misses: 0,
                ocr_latencies: Vec::new(),
                failures: HashMap::new(),
                chunk_sizes: Vec::new(),
            })),
        }
    }

    /// Record engine translation latency
    pub async fn record_engine_latency(&self, engine: &str, ms: u64) {
        let mut inner = self.inner.lock().await;
        inner
            .engine_latencies
            .entry(engine.to_string())
            .or_default()
            .push(ms);
    }

    /// Record cache hit
    pub async fn record_cache_hit(&self) {
        let mut inner = self.inner.lock().await;
        inner.cache_hits += 1;
    }

    /// Record cache miss
    pub async fn record_cache_miss(&self) {
        let mut inner = self.inner.lock().await;
        inner.cache_misses += 1;
    }

    /// Record OCR latency
    pub async fn record_ocr_latency(&self, ms: u64) {
        let mut inner = self.inner.lock().await;
        inner.ocr_latencies.push(ms);
    }

    /// Record translation failure
    pub async fn record_failure(&self, engine: &str, error: &str) {
        let mut inner = self.inner.lock().await;
        inner
            .failures
            .entry(engine.to_string())
            .or_default()
            .push(FailureRecord {
                error: error.to_string(),
                timestamp: Utc::now().timestamp_millis(),
            });
    }

    /// Record document chunk size
    pub async fn record_chunk_size(&self, size: usize) {
        let mut inner = self.inner.lock().await;
        inner.chunk_sizes.push(size);
    }

    /// Get metrics summary
    pub async fn summary(&self) -> MetricsSummary {
        let inner = self.inner.lock().await;

        let engine_stats: HashMap<String, EngineStats> = inner
            .engine_latencies
            .iter()
            .map(|(name, latencies)| {
                let count = latencies.len() as u64;
                let total: u64 = latencies.iter().sum();
                let avg = if count > 0 { total / count } else { 0 };
                let min = latencies.iter().min().copied().unwrap_or(0);
                let max = latencies.iter().max().copied().unwrap_or(0);
                let failures = inner
                    .failures
                    .get(name)
                    .map(|f| f.len() as u64)
                    .unwrap_or(0);

                (
                    name.clone(),
                    EngineStats {
                        count,
                        avg_ms: avg,
                        min_ms: min,
                        max_ms: max,
                        failures,
                    },
                )
            })
            .collect();

        let ocr_stats = if inner.ocr_latencies.is_empty() {
            None
        } else {
            let count = inner.ocr_latencies.len() as u64;
            let total: u64 = inner.ocr_latencies.iter().sum();
            Some(OcrStats {
                count,
                avg_ms: if count > 0 { total / count } else { 0 },
            })
        };

        let cache_stats = CacheStats {
            hits: inner.cache_hits,
            misses: inner.cache_misses,
            hit_rate: if inner.cache_hits + inner.cache_misses > 0 {
                inner.cache_hits as f64 / (inner.cache_hits + inner.cache_misses) as f64
            } else {
                0.0
            },
        };

        let chunk_stats = if inner.chunk_sizes.is_empty() {
            None
        } else {
            let count = inner.chunk_sizes.len();
            let avg = inner.chunk_sizes.iter().sum::<usize>() / count;
            Some(ChunkStats {
                count,
                avg_size: avg,
            })
        };

        MetricsSummary {
            engine_stats,
            ocr_stats,
            cache_stats,
            chunk_stats,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MetricsSummary {
    pub engine_stats: HashMap<String, EngineStats>,
    pub ocr_stats: Option<OcrStats>,
    pub cache_stats: CacheStats,
    pub chunk_stats: Option<ChunkStats>,
}

#[derive(Debug, Serialize)]
pub struct EngineStats {
    pub count: u64,
    pub avg_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub failures: u64,
}

#[derive(Debug, Serialize)]
pub struct OcrStats {
    pub count: u64,
    pub avg_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct ChunkStats {
    pub count: usize,
    pub avg_size: usize,
}

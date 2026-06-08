use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

/// Maximum number of recent latency samples to keep in memory per engine
const RING_BUFFER_SIZE: usize = 1000;
/// Maximum number of failure records to keep in memory per engine
const MAX_FAILURES_IN_MEMORY: usize = 200;
/// Maximum number of timeline entries in SQLite before pruning
const MAX_TIMELINE_ENTRIES: i64 = 100_000;

/// Translation metrics collector with SQLite persistence.
///
/// Uses atomic counters for the hottest paths (cache hits/misses, translation/error counts)
/// to avoid lock contention entirely. Ring buffers and the SQLite connection share a single
/// mutex so that `record_engine_latency` and `record_failure` only need one lock acquisition
/// instead of two.
pub struct MetricsCollector {
    /// Combined state: ring buffers + SQLite connection in a single lock.
    /// This eliminates the double-lock pattern in record_engine_latency/record_failure.
    state: Mutex<MetricsState>,
    /// Lock-free atomic counters for the hottest paths.
    /// cache_hit/miss are called on every translation and only increment,
    /// so atomics eliminate all contention there.
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    total_translations: AtomicU64,
    total_errors: AtomicU64,
}

struct MetricsState {
    /// Engine translation latency ring buffers (engine_name -> circular buffer)
    engine_latencies: HashMap<String, Vec<u64>>,
    /// Ring buffer write index per engine
    engine_write_idx: HashMap<String, usize>,
    /// OCR latency ring buffer
    ocr_latencies: Vec<u64>,
    ocr_write_idx: usize,
    /// Translation failures by engine (recent only)
    failures: HashMap<String, Vec<FailureRecord>>,
    /// Document chunk sizes ring buffer
    chunk_sizes: Vec<usize>,
    chunk_write_idx: usize,
    /// SQLite connection (merged here to avoid a second lock)
    db: Connection,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailureRecord {
    pub error: String,
    pub timestamp: i64,
}

/// A single metrics event persisted to SQLite
#[derive(Debug, Clone, Serialize)]
pub struct MetricsEvent {
    pub id: i64,
    pub event_type: String,
    pub engine: String,
    pub latency_ms: u64,
    pub success: bool,
    pub error_message: String,
    pub timestamp: i64,
}

/// Time-series data point for charts
#[derive(Debug, Clone, Serialize)]
pub struct MetricsTimeline {
    pub timestamp: i64,
    pub engine: String,
    pub latency_ms: u64,
    pub success: bool,
}

fn metrics_db_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!("Failed to create metrics db directory {:?}: {}", path, e);
    }
    path.push("metrics.db");
    path
}

impl MetricsCollector {
    pub fn new() -> Self {
        let db_path = metrics_db_path();
        let conn = match Connection::open(&db_path) {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("Failed to open metrics database: {}", e);
                Connection::open_in_memory().expect("Failed to create in-memory metrics db")
            }
        };

        // Create tables
        if let Err(e) = conn.execute(
            "CREATE TABLE IF NOT EXISTS metrics_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                engine TEXT NOT NULL DEFAULT '',
                latency_ms INTEGER NOT NULL DEFAULT 0,
                success INTEGER NOT NULL DEFAULT 1,
                error_message TEXT NOT NULL DEFAULT '',
                timestamp INTEGER NOT NULL
            )",
            [],
        ) {
            tracing::error!("Failed to create metrics_events table: {}", e);
        }

        if let Err(e) = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics_events(timestamp DESC)",
            [],
        ) {
            tracing::warn!("Failed to create metrics timestamp index: {}", e);
        }

        if let Err(e) = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_metrics_engine ON metrics_events(engine)",
            [],
        ) {
            tracing::warn!("Failed to create metrics engine index: {}", e);
        }

        if let Err(e) = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_metrics_event_type ON metrics_events(event_type)",
            [],
        ) {
            tracing::warn!("Failed to create metrics event_type index: {}", e);
        }

        // Load summary counters from DB
        let (total_translations, total_errors) = Self::load_counters(&conn);

        Self {
            state: Mutex::new(MetricsState {
                engine_latencies: HashMap::new(),
                engine_write_idx: HashMap::new(),
                ocr_latencies: Vec::new(),
                ocr_write_idx: 0,
                failures: HashMap::new(),
                chunk_sizes: Vec::new(),
                chunk_write_idx: 0,
                db: conn,
            }),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            total_translations: AtomicU64::new(total_translations),
            total_errors: AtomicU64::new(total_errors),
        }
    }

    fn load_counters(conn: &Connection) -> (u64, u64) {
        let total_translations = conn
            .query_row(
                "SELECT COUNT(*) FROM metrics_events WHERE event_type = 'translation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0) as u64;

        let total_errors = conn
            .query_row(
                "SELECT COUNT(*) FROM metrics_events WHERE event_type = 'translation' AND success = 0",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0) as u64;

        (total_translations, total_errors)
    }

    /// Record engine translation latency.
    /// Uses a single lock for both the ring buffer update and the SQLite INSERT,
    /// plus one atomic increment for the global counter.
    pub async fn record_engine_latency(&self, engine: &str, ms: u64) {
        let mut state = self.state.lock().await;

        // Update ring buffer
        let engine_key = engine.to_string();
        let current_idx = state.engine_write_idx.get(&engine_key).copied().unwrap_or(0);
        let buffer = state
            .engine_latencies
            .entry(engine_key.clone())
            .or_default();
        let new_idx = if buffer.len() < RING_BUFFER_SIZE {
            buffer.push(ms);
            buffer.len()
        } else {
            buffer[current_idx % RING_BUFFER_SIZE] = ms;
            current_idx + 1
        };
        state.engine_write_idx.insert(engine_key, new_idx);

        // Persist to SQLite (same lock, no second acquisition)
        let timestamp = Utc::now().timestamp_millis();
        if let Err(e) = state.db.execute(
            "INSERT INTO metrics_events (event_type, engine, latency_ms, success, timestamp) VALUES ('translation', ?1, ?2, 1, ?3)",
            params![engine, ms as i64, timestamp],
        ) {
            tracing::warn!("Failed to persist metrics event: {}", e);
        }

        // Atomic increment -- no lock needed
        self.total_translations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record cache hit -- lock-free atomic increment.
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record cache miss -- lock-free atomic increment.
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record OCR latency
    pub async fn record_ocr_latency(&self, ms: u64) {
        let mut state = self.state.lock().await;
        if state.ocr_latencies.len() < RING_BUFFER_SIZE {
            state.ocr_latencies.push(ms);
            state.ocr_write_idx = state.ocr_latencies.len();
        } else {
            let idx = state.ocr_write_idx;
            state.ocr_latencies[idx % RING_BUFFER_SIZE] = ms;
            state.ocr_write_idx = idx + 1;
        }
    }

    /// Record translation failure.
    /// Uses a single lock for both the in-memory failure record and the SQLite INSERT.
    pub async fn record_failure(&self, engine: &str, error: &str) {
        let timestamp = Utc::now().timestamp_millis();
        let mut state = self.state.lock().await;

        let failures = state
            .failures
            .entry(engine.to_string())
            .or_default();
        failures.push(FailureRecord {
            error: error.to_string(),
            timestamp,
        });
        // Trim in-memory failures
        if failures.len() > MAX_FAILURES_IN_MEMORY {
            let drain_count = failures.len() - MAX_FAILURES_IN_MEMORY;
            failures.drain(0..drain_count);
        }

        // Persist to SQLite (same lock, no second acquisition)
        if let Err(e) = state.db.execute(
            "INSERT INTO metrics_events (event_type, engine, latency_ms, success, error_message, timestamp) VALUES ('translation', ?1, 0, 0, ?2, ?3)",
            params![engine, error, timestamp],
        ) {
            tracing::warn!("Failed to persist failure event: {}", e);
        }

        // Atomic increment -- no lock needed
        self.total_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record document chunk size
    pub async fn record_chunk_size(&self, size: usize) {
        let mut state = self.state.lock().await;
        if state.chunk_sizes.len() < RING_BUFFER_SIZE {
            state.chunk_sizes.push(size);
            state.chunk_write_idx = state.chunk_sizes.len();
        } else {
            let idx = state.chunk_write_idx;
            state.chunk_sizes[idx % RING_BUFFER_SIZE] = size;
            state.chunk_write_idx = idx + 1;
        }
    }

    /// Get metrics summary.
    /// Reads atomic counters without any lock, then acquires the single lock
    /// only for ring buffer statistics.
    pub async fn summary(&self) -> MetricsSummary {
        // Read atomics first -- no lock contention
        let total_translations = self.total_translations.load(Ordering::Relaxed);
        let total_errors = self.total_errors.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);

        // Lock once for ring buffer stats
        let state = self.state.lock().await;

        let engine_stats: HashMap<String, EngineStats> = state
            .engine_latencies
            .iter()
            .map(|(name, latencies)| {
                let count = latencies.len() as u64;
                let total: u64 = latencies.iter().sum();
                let avg = if count > 0 { total / count } else { 0 };
                let min = latencies.iter().min().copied().unwrap_or(0);
                let max = latencies.iter().max().copied().unwrap_or(0);
                let p50 = percentile(latencies, 50);
                let p95 = percentile(latencies, 95);
                let p99 = percentile(latencies, 99);
                let failures = state
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
                        p50_ms: p50,
                        p95_ms: p95,
                        p99_ms: p99,
                        failures,
                    },
                )
            })
            .collect();

        let ocr_stats = if state.ocr_latencies.is_empty() {
            None
        } else {
            let count = state.ocr_latencies.len() as u64;
            let total: u64 = state.ocr_latencies.iter().sum();
            Some(OcrStats {
                count,
                avg_ms: if count > 0 { total / count } else { 0 },
            })
        };

        let total_requests = cache_hits + cache_misses;
        let cache_stats = CacheStats {
            hits: cache_hits,
            misses: cache_misses,
            hit_rate: if total_requests > 0 {
                cache_hits as f64 / total_requests as f64
            } else {
                0.0
            },
        };

        let chunk_stats = if state.chunk_sizes.is_empty() {
            None
        } else {
            let count = state.chunk_sizes.len();
            let avg = state.chunk_sizes.iter().sum::<usize>() / count;
            Some(ChunkStats {
                count,
                avg_size: avg,
            })
        };

        let error_rate = if total_translations > 0 {
            total_errors as f64 / total_translations as f64
        } else {
            0.0
        };

        MetricsSummary {
            engine_stats,
            ocr_stats,
            cache_stats,
            chunk_stats,
            total_translations,
            total_errors,
            error_rate,
        }
    }

    /// Get timeline data from SQLite for chart rendering
    pub async fn get_timeline(&self, limit: usize) -> Vec<MetricsTimeline> {
        let state = self.state.lock().await;
        let mut stmt = match state.db.prepare(
            "SELECT timestamp, engine, latency_ms, success FROM metrics_events
             WHERE event_type = 'translation'
             ORDER BY timestamp DESC LIMIT ?1",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("Failed to prepare timeline query: {}", e);
                return Vec::new();
            }
        };

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(MetricsTimeline {
                timestamp: row.get(0)?,
                engine: row.get(1)?,
                latency_ms: row.get::<_, i64>(2)? as u64,
                success: row.get::<_, i32>(3)? != 0,
            })
        });

        match rows {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                tracing::error!("Failed to query timeline: {}", e);
                Vec::new()
            }
        }
    }

    /// Get hourly aggregated stats for the last N hours
    pub async fn get_hourly_stats(&self, hours: i64) -> Vec<HourlyStats> {
        let state = self.state.lock().await;
        let since = Utc::now().timestamp_millis() - hours * 3600 * 1000;

        let mut stmt = match state.db.prepare(
            "SELECT
                (timestamp / 3600000) * 3600000 as hour_ts,
                engine,
                COUNT(*) as total,
                SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as success_count,
                AVG(CASE WHEN success = 1 THEN latency_ms ELSE NULL END) as avg_latency
             FROM metrics_events
             WHERE event_type = 'translation' AND timestamp >= ?1
             GROUP BY hour_ts, engine
             ORDER BY hour_ts DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("Failed to prepare hourly stats query: {}", e);
                return Vec::new();
            }
        };

        let rows = stmt.query_map(params![since], |row| {
            Ok(HourlyStats {
                hour_timestamp: row.get(0)?,
                engine: row.get(1)?,
                total: row.get::<_, i64>(2)? as u64,
                success_count: row.get::<_, i64>(3)? as u64,
                avg_latency_ms: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
            })
        });

        match rows {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                tracing::error!("Failed to query hourly stats: {}", e);
                Vec::new()
            }
        }
    }

    /// Export metrics as CSV string
    pub async fn export_csv(&self) -> String {
        let state = self.state.lock().await;
        let mut stmt = match state.db.prepare(
            "SELECT id, event_type, engine, latency_ms, success, error_message, timestamp
             FROM metrics_events ORDER BY timestamp DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("Failed to prepare CSV export query: {}", e);
                return String::new();
            }
        };

        let mut csv = String::from("id,event_type,engine,latency_ms,success,error_message,timestamp\n");
        let rows = stmt.query_map([], |row| {
            Ok(MetricsEvent {
                id: row.get(0)?,
                event_type: row.get(1)?,
                engine: row.get(2)?,
                latency_ms: row.get::<_, i64>(3)? as u64,
                success: row.get::<_, i32>(4)? != 0,
                error_message: row.get(5)?,
                timestamp: row.get(6)?,
            })
        });

        if let Ok(rows) = rows {
            for row in rows.flatten() {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{}\n",
                    row.id,
                    row.event_type,
                    row.engine,
                    row.latency_ms,
                    row.success,
                    row.error_message.replace(',', ";").replace('\n', " "),
                    row.timestamp
                ));
            }
        }
        csv
    }

    /// Export metrics as JSON
    pub async fn export_json(&self) -> Vec<MetricsEvent> {
        let state = self.state.lock().await;
        let mut stmt = match state.db.prepare(
            "SELECT id, event_type, engine, latency_ms, success, error_message, timestamp
             FROM metrics_events ORDER BY timestamp DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("Failed to prepare JSON export query: {}", e);
                return Vec::new();
            }
        };

        let rows = stmt.query_map([], |row| {
            Ok(MetricsEvent {
                id: row.get(0)?,
                event_type: row.get(1)?,
                engine: row.get(2)?,
                latency_ms: row.get::<_, i64>(3)? as u64,
                success: row.get::<_, i32>(4)? != 0,
                error_message: row.get(5)?,
                timestamp: row.get(6)?,
            })
        });

        match rows {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                tracing::error!("Failed to export JSON: {}", e);
                Vec::new()
            }
        }
    }

    /// Clear all metrics data
    pub async fn clear(&self) {
        // Reset atomics
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.total_translations.store(0, Ordering::Relaxed);
        self.total_errors.store(0, Ordering::Relaxed);

        // Clear DB and ring buffers under single lock
        let mut state = self.state.lock().await;
        if let Err(e) = state.db.execute("DELETE FROM metrics_events", []) {
            tracing::error!("Failed to clear metrics: {}", e);
        }
        state.engine_latencies.clear();
        state.engine_write_idx.clear();
        state.ocr_latencies.clear();
        state.ocr_write_idx = 0;
        state.failures.clear();
        state.chunk_sizes.clear();
        state.chunk_write_idx = 0;
    }

    /// Prune old metrics data (keep last N entries)
    pub async fn prune(&self) {
        let state = self.state.lock().await;
        if let Err(e) = state.db.execute(
            "DELETE FROM metrics_events WHERE id NOT IN (SELECT id FROM metrics_events ORDER BY timestamp DESC LIMIT ?1)",
            params![MAX_TIMELINE_ENTRIES],
        ) {
            tracing::warn!("Failed to prune metrics: {}", e);
        }
    }
}

/// Calculate percentile from data using a stack-allocated buffer for small slices.
/// Avoids heap allocation for typical ring buffer sizes (<= 1000 elements).
fn percentile(data: &[u64], p: u64) -> u64 {
    if data.is_empty() {
        return 0;
    }
    // For typical ring buffer sizes (<=1000), use a small-vec on the stack
    // to avoid a heap allocation on every summary() call.
    let mut buf;
    if data.len() <= 1024 {
        buf = [0u64; 1024];
        buf[..data.len()].copy_from_slice(data);
        buf[..data.len()].sort_unstable();
        let idx = (p as f64 / 100.0 * (data.len() - 1) as f64).round() as usize;
        buf[idx.min(data.len() - 1)]
    } else {
        let mut sorted = data.to_vec();
        sorted.sort_unstable();
        let idx = (p as f64 / 100.0 * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

#[derive(Debug, Serialize)]
pub struct MetricsSummary {
    pub engine_stats: HashMap<String, EngineStats>,
    pub ocr_stats: Option<OcrStats>,
    pub cache_stats: CacheStats,
    pub chunk_stats: Option<ChunkStats>,
    pub total_translations: u64,
    pub total_errors: u64,
    pub error_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct EngineStats {
    pub count: u64,
    pub avg_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
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

#[derive(Debug, Serialize)]
pub struct HourlyStats {
    pub hour_timestamp: i64,
    pub engine: String,
    pub total: u64,
    pub success_count: u64,
    pub avg_latency_ms: f64,
}

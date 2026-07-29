/*!
 * Batch Translation Queue System
 *
 * Manages translation of multiple text segments with:
 * - Configurable concurrency
 * - Progress tracking via events
 * - Cancellation support
 * - Result aggregation
 */

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::services::translation::TranslationService;

/// A single translation task in the batch queue
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTask {
    pub id: String,
    pub index: usize,
    pub text: String,
    pub from_lang: String,
    pub to_lang: String,
    pub status: BatchTaskStatus,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// Status of a batch translation task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BatchTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Overall batch job status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BatchJobStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

/// Progress event sent to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    pub job_id: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub current_index: Option<usize>,
    pub status: BatchJobStatus,
}

/// Configuration for batch translation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchConfig {
    pub concurrency: usize,
    pub from_lang: String,
    pub to_lang: String,
    pub engine: Option<String>,
    pub continue_on_error: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            concurrency: 3,
            from_lang: "auto".to_string(),
            to_lang: "zh".to_string(),
            engine: None,
            continue_on_error: true,
        }
    }
}

/// Batch translation job manager
pub struct BatchManager {
    tasks: Arc<Mutex<VecDeque<BatchTask>>>,
    results: Arc<Mutex<Vec<BatchTask>>>,
    config: Arc<RwLock<BatchConfig>>,
    status: Arc<RwLock<BatchJobStatus>>,
    job_id: Arc<RwLock<Option<String>>>,
    cancel_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    completed_count: Arc<AtomicUsize>,
    failed_count: Arc<AtomicUsize>,
    total_count: Arc<AtomicUsize>,
    app_handle: Arc<RwLock<Option<tauri::AppHandle>>>,
}

impl BatchManager {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
            results: Arc::new(Mutex::new(Vec::new())),
            config: Arc::new(RwLock::new(BatchConfig::default())),
            status: Arc::new(RwLock::new(BatchJobStatus::Idle)),
            job_id: Arc::new(RwLock::new(None)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            completed_count: Arc::new(AtomicUsize::new(0)),
            failed_count: Arc::new(AtomicUsize::new(0)),
            total_count: Arc::new(AtomicUsize::new(0)),
            app_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize with Tauri AppHandle for event emission
    pub fn init(&self, handle: tauri::AppHandle) {
        let app_handle = self.app_handle.clone();
        tokio::spawn(async move {
            *app_handle.write().await = Some(handle);
        });
    }

    /// Submit texts for batch translation
    pub async fn submit(&self, texts: Vec<String>, config: BatchConfig) -> Result<String, String> {
        let mut status = self.status.write().await;
        if *status == BatchJobStatus::Running {
            return Err("A batch job is already running".to_string());
        }

        let job_id = Uuid::new_v4().to_string();
        let total = texts.len();

        // Create tasks
        let mut tasks = VecDeque::with_capacity(total);
        for (i, text) in texts.into_iter().enumerate() {
            tasks.push_back(BatchTask {
                id: Uuid::new_v4().to_string(),
                index: i,
                text,
                from_lang: config.from_lang.clone(),
                to_lang: config.to_lang.clone(),
                status: BatchTaskStatus::Pending,
                result: None,
                error: None,
            });
        }

        // Update state
        *self.tasks.lock().await = tasks;
        self.results.lock().await.clear();
        *self.config.write().await = config;
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.pause_flag.store(false, Ordering::SeqCst);
        self.completed_count.store(0, Ordering::SeqCst);
        self.failed_count.store(0, Ordering::SeqCst);
        self.total_count.store(total, Ordering::SeqCst);
        *self.job_id.write().await = Some(job_id.clone());
        *status = BatchJobStatus::Running;

        // Emit initial progress
        self.emit_progress(&job_id, None).await;

        Ok(job_id)
    }

    /// Start processing the batch queue.
    /// Uses `TranslationService::run_batch` (TM/cache/LLM numbered packs) instead of
    /// per-task `run_full`, with cancel/pause between concurrency-sized waves.
    pub async fn process(&self, service: Arc<TranslationService>) -> Result<(), String> {
        let (concurrency, continue_on_error, from_lang, to_lang) = {
            let cfg = self.config.read().await;
            (
                cfg.concurrency.max(1),
                cfg.continue_on_error,
                cfg.from_lang.clone(),
                cfg.to_lang.clone(),
            )
        };

        loop {
            if self.cancel_flag.load(Ordering::SeqCst) {
                tracing::info!("[Batch] cancelled before next wave");
                break;
            }

            while self.pause_flag.load(Ordering::SeqCst) {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if self.cancel_flag.load(Ordering::SeqCst) {
                    tracing::info!("[Batch] cancelled while paused");
                    return Ok(());
                }
            }

            // Drain one concurrency-sized wave (owned texts for run_batch)
            let wave: Vec<BatchTask> = {
                let mut queue = self.tasks.lock().await;
                let mut taken = Vec::with_capacity(concurrency);
                for _ in 0..concurrency {
                    match queue.pop_front() {
                        Some(mut t) => {
                            t.status = BatchTaskStatus::Running;
                            taken.push(t);
                        },
                        None => break,
                    }
                }
                taken
            };

            if wave.is_empty() {
                break;
            }

            // Ui channel batch → primary + TM/cache (not OCR). Scope lines so wave is free after.
            let by_index: std::collections::HashMap<usize, String> = {
                let lines: Vec<(usize, &str)> =
                    wave.iter().map(|t| (t.index, t.text.as_str())).collect();
                service
                    .run_batch(
                        crate::models::translation::TranslateChannel::Ui,
                        &lines,
                        &from_lang,
                        &to_lang,
                        concurrency,
                    )
                    .await
                    .into_iter()
                    .map(|r| (r.index, r.translated))
                    .collect()
            };

            let mut stop_job = false;
            for mut task in wave {
                let translated = by_index.get(&task.index).cloned().unwrap_or_default();
                let source_empty = task.text.trim().is_empty();
                if !source_empty && translated.trim().is_empty() {
                    task.status = BatchTaskStatus::Failed;
                    task.error = Some("empty translation".to_string());
                    task.result = None;
                    self.failed_count.fetch_add(1, Ordering::SeqCst);
                    if !continue_on_error {
                        tracing::error!(
                            "[Batch] Stopping due to empty translation at {}",
                            task.index
                        );
                        *self.status.write().await = BatchJobStatus::Failed;
                        stop_job = true;
                    }
                } else {
                    task.status = BatchTaskStatus::Completed;
                    task.result = Some(translated);
                    task.error = None;
                    self.completed_count.fetch_add(1, Ordering::SeqCst);
                }

                let task_index = task.index;
                let current_job_id = self.job_id.read().await.clone();
                let completed = self.completed_count.load(Ordering::SeqCst);
                let failed = self.failed_count.load(Ordering::SeqCst);
                let total = self.total_count.load(Ordering::SeqCst);
                let all_done = completed + failed >= total;

                if let Some(jid) = current_job_id {
                    let handle_guard = self.app_handle.read().await;
                    if let Some(handle) = handle_guard.as_ref() {
                        let progress = BatchProgress {
                            job_id: jid,
                            total,
                            completed,
                            failed,
                            current_index: Some(task_index),
                            status: if all_done && !stop_job {
                                BatchJobStatus::Completed
                            } else if stop_job {
                                BatchJobStatus::Failed
                            } else {
                                BatchJobStatus::Running
                            },
                        };
                        let _ = handle.emit("batch-progress", &progress);
                        let _ = handle.emit("batch-task-complete", &task);
                    }
                }

                self.results.lock().await.push(task);

                if all_done && !stop_job {
                    *self.status.write().await = BatchJobStatus::Completed;
                }
                if stop_job {
                    // Leave remaining queue tasks for cancel/reset; mark job failed
                    break;
                }
            }

            if stop_job {
                break;
            }
        }

        // If queue drained cleanly and not cancelled/failed, ensure Completed
        if !self.cancel_flag.load(Ordering::SeqCst) {
            let st = self.status.read().await.clone();
            if st == BatchJobStatus::Running {
                let completed = self.completed_count.load(Ordering::SeqCst);
                let failed = self.failed_count.load(Ordering::SeqCst);
                let total = self.total_count.load(Ordering::SeqCst);
                if completed + failed >= total {
                    *self.status.write().await = BatchJobStatus::Completed;
                }
            }
        }

        Ok(())
    }

    /// Pause the current batch job
    pub async fn pause(&self) -> Result<(), String> {
        let status = self.status.read().await;
        if *status != BatchJobStatus::Running {
            return Err("No running batch job to pause".to_string());
        }
        drop(status);

        self.pause_flag.store(true, Ordering::SeqCst);
        *self.status.write().await = BatchJobStatus::Paused;

        // Emit progress with paused status
        let job_id = self.job_id.read().await.clone();
        if let Some(jid) = job_id {
            self.emit_progress(&jid, None).await;
        }

        Ok(())
    }

    /// Resume a paused batch job
    pub async fn resume(&self) -> Result<(), String> {
        let status = self.status.read().await;
        if *status != BatchJobStatus::Paused {
            return Err("No paused batch job to resume".to_string());
        }
        drop(status);

        self.pause_flag.store(false, Ordering::SeqCst);
        *self.status.write().await = BatchJobStatus::Running;

        // Emit progress with running status
        let job_id = self.job_id.read().await.clone();
        if let Some(jid) = job_id {
            self.emit_progress(&jid, None).await;
        }

        Ok(())
    }

    /// Retry failed tasks
    pub async fn retry_failed(&self, service: Arc<TranslationService>) -> Result<(), String> {
        let status = self.status.read().await;
        if *status != BatchJobStatus::Completed && *status != BatchJobStatus::Failed {
            return Err("Can only retry after completion or failure".to_string());
        }
        drop(status);

        // Collect failed tasks
        let failed_tasks: Vec<BatchTask> = {
            let results = self.results.lock().await;
            results
                .iter()
                .filter(|t| t.status == BatchTaskStatus::Failed)
                .cloned()
                .collect()
        };

        if failed_tasks.is_empty() {
            return Err("No failed tasks to retry".to_string());
        }

        // Reset failed tasks to pending and add back to queue
        {
            let mut tasks = self.tasks.lock().await;
            for mut task in failed_tasks {
                task.status = BatchTaskStatus::Pending;
                task.result = None;
                task.error = None;
                tasks.push_back(task);
            }
        }

        // Decrement failed count
        let retry_count = self.tasks.lock().await.len();
        self.failed_count.fetch_sub(retry_count, Ordering::SeqCst);

        // Update status
        *self.status.write().await = BatchJobStatus::Running;
        self.pause_flag.store(false, Ordering::SeqCst);
        self.cancel_flag.store(false, Ordering::SeqCst);

        // Emit progress
        let job_id = self.job_id.read().await.clone();
        if let Some(jid) = job_id {
            self.emit_progress(&jid, None).await;
        }

        // Start processing
        let batch = self.clone();
        tokio::spawn(async move {
            if let Err(e) = batch.process(service).await {
                tracing::error!("[Batch] Retry processing error: {}", e);
            }
        });

        Ok(())
    }

    /// Cancel the current batch job
    pub async fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        let mut status = self.status.write().await;
        *status = BatchJobStatus::Cancelled;

        // Mark remaining pending tasks as cancelled
        let mut tasks = self.tasks.lock().await;
        while let Some(mut task) = tasks.pop_front() {
            task.status = BatchTaskStatus::Cancelled;
            self.results.lock().await.push(task);
        }

        // Emit final progress
        let job_id = self.job_id.read().await.clone();
        if let Some(jid) = job_id {
            self.emit_progress(&jid, None).await;
        }
    }

    /// Get current batch status
    pub async fn get_status(&self) -> BatchJobStatus {
        self.status.read().await.clone()
    }

    /// Get all results
    pub async fn get_results(&self) -> Vec<BatchTask> {
        self.results.lock().await.clone()
    }

    /// Get progress info
    pub async fn get_progress(&self) -> BatchProgress {
        let job_id = self.job_id.read().await.clone().unwrap_or_default();
        BatchProgress {
            job_id,
            total: self.total_count.load(Ordering::SeqCst),
            completed: self.completed_count.load(Ordering::SeqCst),
            failed: self.failed_count.load(Ordering::SeqCst),
            current_index: None,
            status: self.status.read().await.clone(),
        }
    }

    /// Reset the batch manager
    pub async fn reset(&self) {
        *self.status.write().await = BatchJobStatus::Idle;
        *self.job_id.write().await = None;
        self.tasks.lock().await.clear();
        self.results.lock().await.clear();
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.pause_flag.store(false, Ordering::SeqCst);
        self.completed_count.store(0, Ordering::SeqCst);
        self.failed_count.store(0, Ordering::SeqCst);
        self.total_count.store(0, Ordering::SeqCst);
    }

    async fn emit_progress(&self, job_id: &str, current_index: Option<usize>) {
        let handle_guard = self.app_handle.read().await;
        if let Some(handle) = handle_guard.as_ref() {
            let progress = BatchProgress {
                job_id: job_id.to_string(),
                total: self.total_count.load(Ordering::SeqCst),
                completed: self.completed_count.load(Ordering::SeqCst),
                failed: self.failed_count.load(Ordering::SeqCst),
                current_index,
                status: self.status.read().await.clone(),
            };
            let _ = handle.emit("batch-progress", &progress);
        }
    }
}

impl Clone for BatchManager {
    fn clone(&self) -> Self {
        Self {
            tasks: self.tasks.clone(),
            results: self.results.clone(),
            config: self.config.clone(),
            status: self.status.clone(),
            job_id: self.job_id.clone(),
            cancel_flag: self.cancel_flag.clone(),
            pause_flag: self.pause_flag.clone(),
            completed_count: self.completed_count.clone(),
            failed_count: self.failed_count.clone(),
            total_count: self.total_count.clone(),
            app_handle: self.app_handle.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_config_default() {
        let config = BatchConfig::default();
        assert_eq!(config.concurrency, 3);
        assert_eq!(config.from_lang, "auto");
        assert_eq!(config.to_lang, "zh");
        assert!(config.engine.is_none());
        assert!(config.continue_on_error);
    }

    #[test]
    fn test_batch_task_status_serialization() {
        let status = BatchTaskStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");
    }

    #[test]
    fn test_batch_job_status_serialization() {
        let status = BatchJobStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }

    #[test]
    fn test_batch_job_status_all_variants() {
        let variants = vec![
            (BatchJobStatus::Idle, "\"idle\""),
            (BatchJobStatus::Running, "\"running\""),
            (BatchJobStatus::Paused, "\"paused\""),
            (BatchJobStatus::Completed, "\"completed\""),
            (BatchJobStatus::Cancelled, "\"cancelled\""),
            (BatchJobStatus::Failed, "\"failed\""),
        ];
        for (status, expected) in variants {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected);
        }
    }

    #[test]
    fn test_batch_task_status_all_variants() {
        let variants = vec![
            (BatchTaskStatus::Pending, "\"pending\""),
            (BatchTaskStatus::Running, "\"running\""),
            (BatchTaskStatus::Completed, "\"completed\""),
            (BatchTaskStatus::Failed, "\"failed\""),
            (BatchTaskStatus::Cancelled, "\"cancelled\""),
        ];
        for (status, expected) in variants {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected);
        }
    }

    #[test]
    fn test_batch_manager_creation() {
        let manager = BatchManager::new();
        // Initial status should be Idle
        let status = manager.status.blocking_read().clone();
        assert_eq!(status, BatchJobStatus::Idle);
    }

    #[test]
    fn test_batch_config_custom() {
        let config = BatchConfig {
            concurrency: 5,
            from_lang: "en".to_string(),
            to_lang: "ja".to_string(),
            engine: Some("google".to_string()),
            continue_on_error: false,
        };
        assert_eq!(config.concurrency, 5);
        assert_eq!(config.from_lang, "en");
        assert_eq!(config.to_lang, "ja");
        assert_eq!(config.engine, Some("google".to_string()));
        assert!(!config.continue_on_error);
    }

    #[test]
    fn test_batch_progress_serialization() {
        let progress = BatchProgress {
            job_id: "test-123".to_string(),
            total: 10,
            completed: 5,
            failed: 1,
            current_index: Some(6),
            status: BatchJobStatus::Running,
        };
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"jobId\":\"test-123\""));
        assert!(json.contains("\"total\":10"));
        assert!(json.contains("\"completed\":5"));
        assert!(json.contains("\"failed\":1"));
    }

    #[test]
    fn test_batch_task_creation() {
        let task = BatchTask {
            id: "task-1".to_string(),
            index: 0,
            text: "Hello world".to_string(),
            from_lang: "en".to_string(),
            to_lang: "zh".to_string(),
            status: BatchTaskStatus::Pending,
            result: None,
            error: None,
        };
        assert_eq!(task.id, "task-1");
        assert_eq!(task.index, 0);
        assert_eq!(task.text, "Hello world");
        assert_eq!(task.status, BatchTaskStatus::Pending);
        assert!(task.result.is_none());
        assert!(task.error.is_none());
    }
}

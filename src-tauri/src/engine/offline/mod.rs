//! Offline translation engine backed by the native bergamot-translator stack.
//!
//! Models come from the Mozilla Firefox Model Registry (see `model_catalog`):
//! a downloaded pair lives at `<model_dir>/<from>-<to>/` as decompressed
//! `.bin`/`.spm`/`.s2t.bin` files plus a generated `config.yml` (see
//! `marian_config`). Translation runs through the C ABI bridge (`bridge`),
//! lazily loading models on first use. Pairs without a direct model (e.g.
//! ja→zh) pivot through English via a single chained bridge call.

use super::TranslationEngine;
use anyhow::Context;
use async_trait::async_trait;
use flate2::read::GzDecoder;
use futures::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as AsyncMutex;

pub mod bridge;
pub mod marian_config;
pub mod model_catalog;

use bridge::NativeService;
use model_catalog::ModelSpec;

/// Byte-level progress of a model-pair download, emitted to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub pair: String,
    pub file: usize,
    pub file_total: usize,
    pub done: u64,
    pub total: u64,
}

/// Offline translation engine using local Bergamot models.
pub struct OfflineEngine {
    model_dir: PathBuf,
}

/// Shared native service (one `AsyncService` worker pool per process).
fn shared_service() -> Option<Arc<NativeService>> {
    static SERVICE: std::sync::OnceLock<Option<Arc<NativeService>>> = std::sync::OnceLock::new();
    SERVICE
        .get_or_init(|| NativeService::new(1).ok().map(Arc::new))
        .clone()
}

/// Shared model cache (pair id -> loaded handle), so every `OfflineEngine`
/// instance (Router + commands) reuses loaded models and evictions are global.
fn shared_models() -> &'static AsyncMutex<HashMap<String, Arc<bridge::NativeModel>>> {
    static MODELS: std::sync::OnceLock<AsyncMutex<HashMap<String, Arc<bridge::NativeModel>>>> =
        std::sync::OnceLock::new();
    MODELS.get_or_init(|| AsyncMutex::new(HashMap::new()))
}

impl OfflineEngine {
    pub fn new(model_dir: Option<&str>) -> Self {
        let dir = if let Some(dir) = model_dir {
            PathBuf::from(dir)
        } else {
            let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
            path.push("moontranslator");
            path.push("offline_models");
            path
        };

        if shared_service().is_none() {
            tracing::warn!(
                "[OfflineEngine] native bergamot libs unavailable; engine will report errors on use"
            );
        }

        Self { model_dir: dir }
    }

    /// Get the model directory path.
    pub fn model_dir(&self) -> &PathBuf {
        &self.model_dir
    }

    /// Check whether a pair's model files have been downloaded (config.yml
    /// present under `<model_dir>/<from>-<to>/`).
    pub fn is_model_downloaded(&self, source: &str, target: &str) -> bool {
        let pair = format!("{source}-{target}");
        let Ok(dir) = self.safe_pair_dir(&pair) else {
            return false;
        };
        dir.join("config.yml").exists()
    }

    /// Total size of a downloaded pair's directory.
    pub fn model_size(&self, source: &str, target: &str) -> Option<u64> {
        let pair = format!("{source}-{target}");
        let dir = self.safe_pair_dir(&pair).ok()?;
        if !dir.is_dir() {
            return None;
        }
        let mut total = 0u64;
        let mut entries = std::fs::read_dir(&dir).ok()?;
        while let Some(Ok(entry)) = entries.next() {
            let path = entry.path();
            if path.is_file() {
                total += std::fs::metadata(path).ok()?.len();
            }
        }
        Some(total)
    }

    /// List pair ids that are downloaded.
    pub async fn available_pairs(&self) -> Vec<String> {
        let mut pairs = Vec::new();
        let Ok(mut entries) = tokio::fs::read_dir(&self.model_dir).await else {
            return pairs;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() && path.join("config.yml").exists() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    pairs.push(name.to_string());
                }
            }
        }
        pairs.sort();
        pairs
    }

    /// Download + verify a pair's model files and write its `config.yml`.
    ///
    /// `on_progress` is invoked as chunks arrive (per-file byte counts). Each
    /// file is streamed to a `.part` temp file, then gunzipped into place (and
    /// the model binary SHA-256-verified against the registry) before the next
    /// file starts. Already-present files are skipped, so re-runs resume.
    pub async fn download_model(
        &self,
        pair_id: &str,
        on_progress: Option<impl Fn(DownloadProgress)>,
    ) -> anyhow::Result<()> {
        let spec = model_catalog::model_spec_by_id(pair_id)
            .ok_or_else(|| anyhow::anyhow!("unknown model pair: {pair_id}"))?;
        let dir = self.safe_pair_dir(pair_id)?;
        tokio::fs::create_dir_all(&dir).await?;

        let client = reqwest::Client::new();
        let file_total = spec.files.len();
        for (i, file) in spec.files.iter().enumerate() {
            let target_name = file.name.strip_suffix(".gz").unwrap_or(&file.name);
            let dest = dir.join(target_name);
            if dest.exists() {
                tracing::info!("[OfflineEngine] {pair_id}/{target_name} already present, skipping");
                continue;
            }
            tracing::info!("[OfflineEngine] downloading {pair_id}/{}", file.name);

            let resp = client
                .get(&file.url)
                .send()
                .await
                .with_context(|| format!("GET {} failed", file.url))?
                .error_for_status()
                .with_context(|| format!("HTTP error for {}", file.url))?;
            let total = resp.content_length().unwrap_or(file.size_bytes).max(1);
            let tmp = dir.join(format!("{target_name}.part"));
            let mut stream = resp.bytes_stream();
            let mut out = tokio::fs::File::create(&tmp).await.context("create .part file")?;
            let mut done = 0u64;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.with_context(|| format!("read chunk for {}", file.name))?;
                done += chunk.len() as u64;
                out.write_all(&chunk).await.context("write chunk")?;
                if let Some(cb) = &on_progress {
                    cb(DownloadProgress {
                        pair: pair_id.to_string(),
                        file: i,
                        file_total,
                        done,
                        total,
                    });
                }
            }
            out.flush().await.context("flush .part file")?;
            drop(out);

            let content = if file.name.to_ascii_lowercase().ends_with(".gz") {
                let file = std::fs::File::open(&tmp).context("open .part file")?;
                let mut decoder = GzDecoder::new(file);
                let mut bytes = Vec::new();
                decoder.read_to_end(&mut bytes).context("gunzip download")?;
                if target_name.starts_with("model.") {
                    verify_sha256(&bytes, &spec.sha256, target_name)?;
                }
                bytes
            } else {
                tokio::fs::read(&tmp).await.context("read .part file")?
            };
            tokio::fs::write(&dest, &content).await.context("write final file")?;
            tokio::fs::remove_file(&tmp).await.ok();
        }

        let config = marian_config::build_config(&spec);
        tokio::fs::write(dir.join("config.yml"), config).await?;
        tracing::info!("[OfflineEngine] downloaded model pair {pair_id}");
        Ok(())
    }

    /// Delete a downloaded pair's directory and evict it from the cache.
    pub async fn delete_model(&self, source: &str, target: &str) -> anyhow::Result<()> {
        let pair = format!("{source}-{target}");
        let dir = self.safe_pair_dir(&pair)?;
        shared_models().lock().await.remove(&pair);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await?;
        }
        tracing::info!("[OfflineEngine] deleted model pair {pair}");
        Ok(())
    }

    /// Resolve the catalog entry for a pair id (frontend listing).
    pub fn catalog_entry(pair_id: &str) -> Option<ModelSpec> {
        model_catalog::model_spec_by_id(pair_id)
    }

    /// All catalog entries (frontend listing).
    pub fn catalog_entries() -> Vec<ModelSpec> {
        model_catalog::registry_entries()
    }

    /// Resolve `<model_dir>/<pair_id>` only when `pair_id` cannot escape the
    /// model directory (guards command inputs like `source`/`target` against
    /// path traversal such as `../..` or `..\..`).
    fn safe_pair_dir(&self, pair_id: &str) -> anyhow::Result<PathBuf> {
        if pair_id.is_empty()
            || pair_id.contains("..")
            || pair_id.contains('/')
            || pair_id.contains('\\')
            || !pair_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            anyhow::bail!("invalid model pair id: {pair_id:?}");
        }
        let dir = self.model_dir.join(pair_id);
        let base = self
            .model_dir
            .canonicalize()
            .unwrap_or_else(|_| self.model_dir.clone());
        let resolved = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if !resolved.starts_with(&base) {
            anyhow::bail!("model pair escapes model dir: {pair_id:?}");
        }
        Ok(dir)
    }
}

/// Verify uncompressed model bytes against the registry SHA-256.
fn verify_sha256(content: &[u8], expected: &str, name: &str) -> anyhow::Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let digest = hasher.finalize();
    let mut actual = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(actual, "{byte:02x}")?;
    }
    if actual != expected {
        anyhow::bail!("sha256 mismatch for {name}: got {actual}, expected {expected}");
    }
    Ok(())
}

#[async_trait]
impl TranslationEngine for OfflineEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let chain = model_catalog::translation_chain(from, to)
            .ok_or_else(|| anyhow::anyhow!("no offline model for {from} -> {to}"))?;

        let service = shared_service()
            .ok_or_else(|| anyhow::anyhow!("native bergamot libs are not built"))?;
        let model_dir = self.model_dir.clone();
        let text = text.to_string();
        let from = from.to_string();
        let to = to.to_string();

        tokio::task::spawn_blocking(move || {
            match chain.as_slice() {
                [pair] => {
                    let model = load_model(&model_dir, &service, pair)?;
                    service.translate(model.as_ref(), &text)
                },
                [first, second] => {
                    let m1 = load_model(&model_dir, &service, first)?;
                    let m2 = load_model(&model_dir, &service, second)?;
                    service.pivot(m1.as_ref(), m2.as_ref(), &text)
                },
                _ => anyhow::bail!("unsupported chain length for {from} -> {to}"),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("offline translate task failed: {e}"))?
    }

    fn name(&self) -> &'static str {
        "Offline"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Load (or reuse) a pair's model. Blocking: call from `spawn_blocking`.
fn load_model(
    model_dir: &std::path::Path,
    svc: &NativeService,
    pair_id: &str,
) -> anyhow::Result<Arc<bridge::NativeModel>> {
    let cache = shared_models();
    {
        let cache = cache.blocking_lock();
        if let Some(model) = cache.get(pair_id) {
            return Ok(Arc::clone(model));
        }
    }
    let config_path = model_dir.join(pair_id).join("config.yml");
    if !config_path.exists() {
        anyhow::bail!(
            "model `{pair_id}` is not downloaded (missing `{}`)",
            config_path.display()
        );
    }
    let model = Arc::new(svc.load_model(&config_path.to_string_lossy())?);
    cache
        .blocking_lock()
        .insert(pair_id.to_string(), Arc::clone(&model));
    Ok(model)
}

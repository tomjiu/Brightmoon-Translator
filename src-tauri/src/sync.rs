//! Cloud sync via WebDAV.
//!
//! Syncs config, glossary, translation memory (history DB), and wordbook
//! to a WebDAV server (e.g., Nutstore, NextCloud, etc.).

use crate::config::AppConfig;
use crate::error::AppError;
use crate::glossary::Glossary;
use crate::memory::{HistoryStore, TmExportData, WordBookStore};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Sync data manifest — describes what files exist on the remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncManifest {
    pub version: u32,
    pub device_id: String,
    pub updated_at: i64,
    pub files: Vec<SyncFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFileEntry {
    pub name: String,
    pub size: u64,
    pub checksum: String,
    pub updated_at: i64,
}

/// Status returned to the frontend after a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub success: bool,
    pub message: String,
    pub synced_at: i64,
    pub uploaded: Vec<String>,
    pub downloaded: Vec<String>,
}

/// Get or create a stable device ID for this installation.
pub fn device_id() -> String {
    let path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("moontranslator")
        .join("device_id");
    if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_else(|_| generate_device_id())
    } else {
        let id = generate_device_id();
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
        let _ = std::fs::write(&path, &id);
        id
    }
}

fn generate_device_id() -> String {
    format!("mt-{}", uuid::Uuid::new_v4())
}

/// Compute MD5 hex checksum of bytes.
fn checksum(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

/// Build a reqwest client with optional proxy from config.
fn build_client(config: &AppConfig) -> Result<reqwest::Client, AppError> {
    let mut builder = config.proxy.to_client_builder();
    builder = builder.timeout(std::time::Duration::from_secs(config.http_timeout_secs));
    builder.build().map_err(|e| AppError::Network(e.to_string()))
}

/// Base URL for the WebDAV remote directory, ensuring trailing slash.
fn remote_base_url(config: &AppConfig) -> String {
    let base = config.sync.server_url.trim_end_matches('/');
    let dir = config.sync.remote_dir.trim_matches('/');
    format!("{}/{}", base, dir)
}

/// PUT upload a file to WebDAV.
async fn upload_file(
    client: &reqwest::Client,
    config: &AppConfig,
    remote_path: &str,
    data: Vec<u8>,
) -> Result<(), AppError> {
    let url = format!(
        "{}/{}",
        remote_base_url(config),
        remote_path.trim_start_matches('/')
    );
    let resp = client
        .put(&url)
        .basic_auth(&config.sync.username, Some(&config.sync.password))
        .header("Content-Type", "application/octet-stream")
        .body(data)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::http_status(status, &body));
    }
    Ok(())
}

/// GET download a file from WebDAV.
async fn download_file(
    client: &reqwest::Client,
    config: &AppConfig,
    remote_path: &str,
) -> Result<Vec<u8>, AppError> {
    let url = format!(
        "{}/{}",
        remote_base_url(config),
        remote_path.trim_start_matches('/')
    );
    let resp = client
        .get(&url)
        .basic_auth(&config.sync.username, Some(&config.sync.password))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::http_status(status, &body));
    }
    Ok(resp.bytes().await?.to_vec())
}

/// MKCOL — create directory on WebDAV (ignore if already exists).
async fn ensure_remote_dir(
    client: &reqwest::Client,
    config: &AppConfig,
) -> Result<(), AppError> {
    let url = format!("{}/", remote_base_url(config));
    let resp = client
        .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &url)
        .basic_auth(&config.sync.username, Some(&config.sync.password))
        .send()
        .await?;

    // 201 Created or 405 Method Not Allowed (already exists) are both OK
    let status = resp.status().as_u16();
    if status == 201 || status == 405 || status == 301 || status == 200 {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(AppError::http_status(status, &body))
    }
}

/// Test WebDAV connection. Returns Ok(()) on success.
pub async fn test_connection(config: &AppConfig) -> Result<(), AppError> {
    if config.sync.server_url.is_empty() {
        return Err(AppError::Config("WebDAV server URL is empty".into()));
    }
    if config.sync.username.is_empty() {
        return Err(AppError::Config("WebDAV username is empty".into()));
    }

    let client = build_client(config)?;
    ensure_remote_dir(&client, config).await?;
    Ok(())
}

/// Perform a full bidirectional sync.
///
/// Strategy: "last-write-wins" per file. For each data type (config, glossary,
/// history, wordbook), compare local timestamp vs remote manifest timestamp.
/// Upload if local is newer, download if remote is newer.
pub async fn sync_all(
    config: &AppConfig,
    glossary: Arc<Mutex<Glossary>>,
    history: Arc<Mutex<HistoryStore>>,
    wordbook: Arc<Mutex<WordBookStore>>,
) -> Result<SyncStatus, AppError> {
    if !config.sync.enabled {
        return Err(AppError::Config("Cloud sync is not enabled".into()));
    }

    let client = build_client(config)?;
    ensure_remote_dir(&client, config).await?;

    let now = chrono::Utc::now().timestamp_millis();
    let mut uploaded = Vec::new();
    let mut downloaded = Vec::new();

    // Try to download existing manifest
    let remote_manifest = match download_file(&client, config, "manifest.json").await {
        Ok(data) => serde_json::from_str::<SyncManifest>(&String::from_utf8_lossy(&data)).ok(),
        Err(_) => None,
    };

    // Build local manifest
    let local_manifest = build_local_manifest(config, &glossary, &history, &wordbook).await;

    // === Sync Config ===
    if config.sync.sync_config {
        let local_entry = local_manifest.files.iter().find(|f| f.name == "config.json");
        let remote_entry = remote_manifest.as_ref().and_then(|m| m.files.iter().find(|f| f.name == "config.json"));

        match (local_entry, remote_entry) {
            (Some(local), Some(remote)) if local.updated_at > remote.updated_at => {
                // Upload local config
                let config_data = serialize_config_for_sync(config)?;
                upload_file(&client, config, "config.json", config_data.into_bytes()).await?;
                uploaded.push("config.json".to_string());
            }
            (Some(_), Some(_remote)) => {
                // Download remote config
                let data = download_file(&client, config, "config.json").await?;
                let json = String::from_utf8_lossy(&data).to_string();
                // Store downloaded config for the caller to apply
                downloaded.push(format!("config.json:{}", json.len()));
            }
            (Some(_local), None) => {
                // Upload local (no remote exists)
                let config_data = serialize_config_for_sync(config)?;
                upload_file(&client, config, "config.json", config_data.into_bytes()).await?;
                uploaded.push("config.json".to_string());
            }
            (None, Some(_)) => {
                // Download remote (no local equivalent in manifest)
                let data = download_file(&client, config, "config.json").await?;
                downloaded.push(format!("config.json:{}", data.len()));
            }
            _ => {}
        }
    }

    // === Sync Glossary ===
    if config.sync.sync_glossary {
        let local_entry = local_manifest.files.iter().find(|f| f.name == "glossary.json");
        let remote_entry = remote_manifest.as_ref().and_then(|m| m.files.iter().find(|f| f.name == "glossary.json"));

        let should_upload = match (local_entry, remote_entry) {
            (Some(local), Some(remote)) => local.updated_at > remote.updated_at,
            (Some(_), None) => true,
            _ => false,
        };

        if should_upload {
            let glossary = glossary.lock().await;
            let data = serde_json::to_string_pretty(glossary.get_all_entries())
                .map_err(|e| AppError::Internal(e.to_string()))?;
            upload_file(&client, config, "glossary.json", data.into_bytes()).await?;
            uploaded.push("glossary.json".to_string());
        } else if remote_entry.is_some() {
            let data = download_file(&client, config, "glossary.json").await?;
            let entries: std::collections::HashMap<String, Vec<crate::models::glossary::GlossaryEntry>> =
                serde_json::from_slice(&data).unwrap_or_default();
            let mut g = glossary.lock().await;
            // Merge remote entries into local
            for (lang_pair, remote_entries) in entries {
                for entry in remote_entries {
                    let local_entries = g.get_entries(&lang_pair);
                    let exists = local_entries.iter().any(|e| e.source == entry.source);
                    if !exists {
                        g.add_entry(lang_pair.clone(), entry).await;
                    }
                }
            }
            downloaded.push("glossary.json".to_string());
        }
    }

    // === Sync History (Translation Memory) ===
    if config.sync.sync_history {
        let local_entry = local_manifest.files.iter().find(|f| f.name == "tm_export.json");
        let remote_entry = remote_manifest.as_ref().and_then(|m| m.files.iter().find(|f| f.name == "tm_export.json"));

        let should_upload = match (local_entry, remote_entry) {
            (Some(local), Some(remote)) => local.updated_at > remote.updated_at,
            (Some(_), None) => true,
            _ => false,
        };

        if should_upload {
            let history = history.lock().await;
            let export = history.export_tm(None, None);
            let data = serde_json::to_string(&export)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            upload_file(&client, config, "tm_export.json", data.into_bytes()).await?;
            uploaded.push("tm_export.json".to_string());
        } else if remote_entry.is_some() {
            let data = download_file(&client, config, "tm_export.json").await?;
            let export: TmExportData = serde_json::from_slice(&data)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let h = history.lock().await;
            let (imported, _skipped) = h.import_tm(&export, true);
            downloaded.push(format!("tm_export.json ({} entries)", imported));
        }
    }

    // === Sync Wordbook ===
    if config.sync.sync_wordbook {
        let local_entry = local_manifest.files.iter().find(|f| f.name == "wordbook.json");
        let remote_entry = remote_manifest.as_ref().and_then(|m| m.files.iter().find(|f| f.name == "wordbook.json"));

        let should_upload = match (local_entry, remote_entry) {
            (Some(local), Some(remote)) => local.updated_at > remote.updated_at,
            (Some(_), None) => true,
            _ => false,
        };

        if should_upload {
            let wb = wordbook.lock().await;
            let items = wb.get_all();
            let data = serde_json::to_string_pretty(&items)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            upload_file(&client, config, "wordbook.json", data.into_bytes()).await?;
            uploaded.push("wordbook.json".to_string());
        } else if remote_entry.is_some() {
            let data = download_file(&client, config, "wordbook.json").await?;
            let items: Vec<crate::memory::WordBookItem> = serde_json::from_slice(&data)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let wb = wordbook.lock().await;
            for item in &items {
                let _ = wb.add(&item.word, &item.translation, &item.from_lang, &item.to_lang, &item.note);
            }
            downloaded.push(format!("wordbook.json ({} entries)", items.len()));
        }
    }

    // Upload updated manifest
    let new_manifest = SyncManifest {
        version: 1,
        device_id: device_id(),
        updated_at: now,
        files: build_local_manifest(config, &glossary, &history, &wordbook).await.files,
    };
    let manifest_json = serde_json::to_string_pretty(&new_manifest)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    upload_file(&client, config, "manifest.json", manifest_json.into_bytes()).await?;

    let message = if uploaded.is_empty() && downloaded.is_empty() {
        "Already up to date".to_string()
    } else {
        format!(
            "Uploaded: {}, Downloaded: {}",
            uploaded.len(),
            downloaded.len()
        )
    };

    Ok(SyncStatus {
        success: true,
        message,
        synced_at: now,
        uploaded,
        downloaded,
    })
}

/// Build a local manifest from current data state.
async fn build_local_manifest(
    config: &AppConfig,
    glossary: &Arc<Mutex<Glossary>>,
    history: &Arc<Mutex<HistoryStore>>,
    wordbook: &Arc<Mutex<WordBookStore>>,
) -> SyncManifest {
    let now = chrono::Utc::now().timestamp_millis();
    let mut files = Vec::new();

    // Config
    if let Ok(data) = serialize_config_for_sync(config) {
        files.push(SyncFileEntry {
            name: "config.json".to_string(),
            size: data.len() as u64,
            checksum: checksum(data.as_bytes()),
            updated_at: config.sync.last_sync_at.max(now),
        });
    }

    // Glossary
    {
        let g = glossary.lock().await;
        if let Ok(data) = serde_json::to_string(g.get_all_entries()) {
            files.push(SyncFileEntry {
                name: "glossary.json".to_string(),
                size: data.len() as u64,
                checksum: checksum(data.as_bytes()),
                updated_at: now,
            });
        }
    }

    // History (TM)
    {
        let h = history.lock().await;
        let export = h.export_tm(None, None);
        if let Ok(data) = serde_json::to_string(&export) {
            files.push(SyncFileEntry {
                name: "tm_export.json".to_string(),
                size: data.len() as u64,
                checksum: checksum(data.as_bytes()),
                updated_at: now,
            });
        }
    }

    // Wordbook
    {
        let wb = wordbook.lock().await;
        let items = wb.get_all();
        if let Ok(data) = serde_json::to_string(&items) {
            files.push(SyncFileEntry {
                name: "wordbook.json".to_string(),
                size: data.len() as u64,
                checksum: checksum(data.as_bytes()),
                updated_at: now,
            });
        }
    }

    SyncManifest {
        version: 1,
        device_id: device_id(),
        updated_at: now,
        files,
    }
}

/// Serialize config for sync (with secrets masked to avoid leaking to cloud).
fn serialize_config_for_sync(config: &AppConfig) -> Result<String, AppError> {
    let masked = config.masked_copy();
    serde_json::to_string_pretty(&masked).map_err(|e| AppError::Internal(e.to_string()))
}

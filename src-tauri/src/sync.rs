//! Cloud sync via `WebDAV`.
//!
//! Syncs config, glossary, translation memory (history DB), and wordbook
//! to a `WebDAV` server (e.g., Nutstore, `NextCloud`, etc.).

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
    /// Remote config.json body when downloaded (caller applies / merges).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloaded_config: Option<String>,
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

/// Magic header for encrypted sync blobs (AES-256-GCM wire format).
const SYNC_ENC_MAGIC: &[u8] = b"MTS1";

/// Derive 32-byte key: `SHA256(device_id` ‖ password ‖ "moontranslator-sync-v1").
/// Password from `WebDAV` enables multi-device; empty password = device-local only.
fn derive_sync_key(password: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(device_id().as_bytes());
    hasher.update(password.as_bytes());
    hasher.update(b"moontranslator-sync-v1");
    let hash = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash);
    key
}

fn encrypt_sync_payload(plaintext: &[u8], password: &str) -> Result<Vec<u8>, AppError> {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
    use rand::RngCore;

    let key_bytes = derive_sync_key(password);
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| AppError::Internal(format!("sync encrypt: {e}")))?;

    let mut out = Vec::with_capacity(SYNC_ENC_MAGIC.len() + 12 + ciphertext.len());
    out.extend_from_slice(SYNC_ENC_MAGIC);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt if magic present; otherwise treat as legacy plaintext JSON.
fn decrypt_sync_payload(data: &[u8], password: &str) -> Result<Vec<u8>, AppError> {
    if data.len() >= SYNC_ENC_MAGIC.len() && data.starts_with(SYNC_ENC_MAGIC) {
        use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
        let body = &data[SYNC_ENC_MAGIC.len()..];
        if body.len() < 12 + 16 {
            return Err(AppError::Internal("sync ciphertext too short".into()));
        }
        let key_bytes = derive_sync_key(password);
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let (nonce_bytes, ciphertext) = body.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        return cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::Internal(format!("sync decrypt failed (wrong password/device?): {e}")));
    }
    // Legacy plaintext
    Ok(data.to_vec())
}

/// Build a reqwest client with optional proxy from config.
fn build_client(config: &AppConfig) -> Result<reqwest::Client, AppError> {
    let mut builder = config.proxy.to_client_builder();
    builder = builder.timeout(std::time::Duration::from_secs(config.http_timeout_secs));
    builder
        .build()
        .map_err(|e| AppError::Network(e.to_string()))
}

/// Base URL for the `WebDAV` remote directory, ensuring trailing slash.
fn remote_base_url(config: &AppConfig) -> String {
    let base = config.sync.server_url.trim_end_matches('/');
    let dir = config.sync.remote_dir.trim_matches('/');
    format!("{base}/{dir}")
}

/// PUT upload a file to `WebDAV`.
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

/// GET download a file from `WebDAV`.
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

/// MKCOL — create directory on `WebDAV` (ignore if already exists).
async fn ensure_remote_dir(client: &reqwest::Client, config: &AppConfig) -> Result<(), AppError> {
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

/// Test `WebDAV` connection. Returns Ok(()) on success.
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
    let mut downloaded_config: Option<String> = None;
    let sync_pw = config.sync.password.as_str();

    // Try to download existing manifest
    let remote_manifest = match download_file(&client, config, "manifest.json").await {
        Ok(data) => serde_json::from_str::<SyncManifest>(&String::from_utf8_lossy(&data)).ok(),
        Err(_) => None,
    };

    // Build local manifest
    let local_manifest = build_local_manifest(config, &glossary, &history, &wordbook).await;

    // === Sync Config ===
    if config.sync.sync_config {
        let local_entry = local_manifest
            .files
            .iter()
            .find(|f| f.name == "config.json");
        let remote_entry = remote_manifest
            .as_ref()
            .and_then(|m| m.files.iter().find(|f| f.name == "config.json"));

        match (local_entry, remote_entry) {
            (Some(local), Some(remote)) if local.updated_at > remote.updated_at => {
                let config_data = serialize_config_for_sync(config)?;
                upload_file(&client, config, "config.json", config_data.into_bytes()).await?;
                uploaded.push("config.json".to_string());
            },
            (Some(_), Some(_remote)) => {
                let data = download_file(&client, config, "config.json").await?;
                let json = String::from_utf8_lossy(&data).to_string();
                downloaded.push("config.json".to_string());
                downloaded_config = Some(json);
            },
            (Some(_local), None) => {
                let config_data = serialize_config_for_sync(config)?;
                upload_file(&client, config, "config.json", config_data.into_bytes()).await?;
                uploaded.push("config.json".to_string());
            },
            (None, Some(_)) => {
                let data = download_file(&client, config, "config.json").await?;
                let json = String::from_utf8_lossy(&data).to_string();
                downloaded.push("config.json".to_string());
                downloaded_config = Some(json);
            },
            _ => {},
        }
    }

    // === Sync Glossary (AES-GCM) ===
    if config.sync.sync_glossary {
        let local_entry = local_manifest
            .files
            .iter()
            .find(|f| f.name == "glossary.json");
        let remote_entry = remote_manifest
            .as_ref()
            .and_then(|m| m.files.iter().find(|f| f.name == "glossary.json"));

        let should_upload = match (local_entry, remote_entry) {
            (Some(local), Some(remote)) => local.updated_at > remote.updated_at,
            (Some(_), None) => true,
            _ => false,
        };

        if should_upload {
            let glossary = glossary.lock().await;
            let data = serde_json::to_string_pretty(glossary.get_all_entries())
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let enc = encrypt_sync_payload(data.as_bytes(), sync_pw)?;
            upload_file(&client, config, "glossary.json", enc).await?;
            uploaded.push("glossary.json".to_string());
        } else if remote_entry.is_some() {
            let raw = download_file(&client, config, "glossary.json").await?;
            let data = decrypt_sync_payload(&raw, sync_pw)?;
            let entries: std::collections::HashMap<
                String,
                Vec<crate::models::glossary::GlossaryEntry>,
            > = serde_json::from_slice(&data).unwrap_or_default();
            let mut g = glossary.lock().await;
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

    // === Sync History (Translation Memory, AES-GCM) ===
    if config.sync.sync_history {
        let local_entry = local_manifest
            .files
            .iter()
            .find(|f| f.name == "tm_export.json");
        let remote_entry = remote_manifest
            .as_ref()
            .and_then(|m| m.files.iter().find(|f| f.name == "tm_export.json"));

        let should_upload = match (local_entry, remote_entry) {
            (Some(local), Some(remote)) => local.updated_at > remote.updated_at,
            (Some(_), None) => true,
            _ => false,
        };

        if should_upload {
            let history = history.lock().await;
            let export = history.export_tm(None, None);
            let data =
                serde_json::to_string(&export).map_err(|e| AppError::Internal(e.to_string()))?;
            let enc = encrypt_sync_payload(data.as_bytes(), sync_pw)?;
            upload_file(&client, config, "tm_export.json", enc).await?;
            uploaded.push("tm_export.json".to_string());
        } else if remote_entry.is_some() {
            let raw = download_file(&client, config, "tm_export.json").await?;
            let data = decrypt_sync_payload(&raw, sync_pw)?;
            let export: TmExportData =
                serde_json::from_slice(&data).map_err(|e| AppError::Internal(e.to_string()))?;
            let h = history.lock().await;
            let (imported, _skipped) = h.import_tm(&export, true);
            downloaded.push(format!("tm_export.json ({imported} entries)"));
        }
    }

    // === Sync Wordbook (AES-GCM) ===
    if config.sync.sync_wordbook {
        let local_entry = local_manifest
            .files
            .iter()
            .find(|f| f.name == "wordbook.json");
        let remote_entry = remote_manifest
            .as_ref()
            .and_then(|m| m.files.iter().find(|f| f.name == "wordbook.json"));

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
            let enc = encrypt_sync_payload(data.as_bytes(), sync_pw)?;
            upload_file(&client, config, "wordbook.json", enc).await?;
            uploaded.push("wordbook.json".to_string());
        } else if remote_entry.is_some() {
            let raw = download_file(&client, config, "wordbook.json").await?;
            let data = decrypt_sync_payload(&raw, sync_pw)?;
            let items: Vec<crate::memory::WordBookItem> =
                serde_json::from_slice(&data).map_err(|e| AppError::Internal(e.to_string()))?;
            let wb = wordbook.lock().await;
            for item in &items {
                let _ = wb.add(
                    &item.word,
                    &item.translation,
                    &item.from_lang,
                    &item.to_lang,
                    &item.note,
                );
            }
            downloaded.push(format!("wordbook.json ({} entries)", items.len()));
        }
    }

    // Upload updated manifest
    let new_manifest = SyncManifest {
        version: 1,
        device_id: device_id(),
        updated_at: now,
        files: build_local_manifest(config, &glossary, &history, &wordbook)
            .await
            .files,
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
        downloaded_config,
    })
}

#[cfg(test)]
mod crypto_tests {
    use super::*;

    #[test]
    fn sync_payload_roundtrip() {
        let plain = br#"{"en-zh":[{"source":"hello","target":"nihao"}]}"#;
        let enc = encrypt_sync_payload(plain, "test-password").unwrap();
        assert!(enc.starts_with(SYNC_ENC_MAGIC));
        assert_ne!(enc, plain);
        let dec = decrypt_sync_payload(&enc, "test-password").unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn legacy_plaintext_still_loads() {
        let plain = b"{\"ok\":true}";
        let dec = decrypt_sync_payload(plain, "any").unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn wrong_password_fails() {
        let enc = encrypt_sync_payload(b"secret-data-here", "pw-a").unwrap();
        assert!(decrypt_sync_payload(&enc, "pw-b").is_err());
    }
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

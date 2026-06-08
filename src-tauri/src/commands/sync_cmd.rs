use crate::error::AppError;
use crate::sync::{self, SyncStatus};
use crate::AppState;
use tauri::State;

fn validate_sync_inputs(
    server_url: &str,
    remote_dir: &str,
    interval_mins: u64,
) -> Result<(), AppError> {
    let parsed = reqwest::Url::parse(server_url)
        .map_err(|e| AppError::Config(format!("Invalid sync server URL: {}", e)))?;
    match parsed.scheme() {
        "http" | "https" => {},
        _ => {
            return Err(AppError::Config(
                "Sync server URL must use http or https".to_string(),
            ));
        },
    }

    if parsed.host_str().is_none() {
        return Err(AppError::Config(
            "Sync server URL must include a host".to_string(),
        ));
    }

    if remote_dir.trim().is_empty()
        || remote_dir.contains("..")
        || remote_dir.contains('\\')
        || remote_dir.contains('\0')
    {
        return Err(AppError::Config(
            "Sync remote directory is invalid".to_string(),
        ));
    }

    if interval_mins == 0 || interval_mins > 24 * 60 {
        return Err(AppError::Config(
            "Sync interval must be between 1 and 1440 minutes".to_string(),
        ));
    }

    Ok(())
}

/// Test WebDAV connection with current sync config.
#[tauri::command]
pub async fn test_webdav_connection(state: State<'_, AppState>) -> Result<String, AppError> {
    let config = state.system.config.lock().await;
    sync::test_connection(&config).await?;
    Ok("Connection successful".to_string())
}

/// Run a manual sync now.
#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>) -> Result<SyncStatus, AppError> {
    let config = state.system.config.lock().await;
    if !config.sync.enabled {
        return Err(AppError::Config("Cloud sync is not enabled".into()));
    }
    let glossary = state.translation.glossary.clone();
    let history = state.document.history.clone();
    let wordbook = state.document.wordbook.clone();
    let sync_config = config.clone();
    drop(config);

    let result = sync::sync_all(&sync_config, glossary, history, wordbook).await?;

    // Update last sync status in config
    let mut config = state.system.config.lock().await;
    config.sync.last_sync_at = result.synced_at;
    config.sync.last_sync_status = if result.success {
        result.message.clone()
    } else {
        format!("Failed: {}", result.message)
    };
    config.save();

    Ok(result)
}

/// Get current sync configuration.
#[tauri::command]
pub async fn get_sync_config(state: State<'_, AppState>) -> Result<serde_json::Value, AppError> {
    let config = state.system.config.lock().await;
    let sync = &config.sync;
    Ok(serde_json::json!({
        "enabled": sync.enabled,
        "serverUrl": sync.server_url,
        "username": sync.username,
        "remoteDir": sync.remote_dir,
        "intervalMins": sync.interval_mins,
        "syncConfig": sync.sync_config,
        "syncGlossary": sync.sync_glossary,
        "syncHistory": sync.sync_history,
        "syncWordbook": sync.sync_wordbook,
        "lastSyncAt": sync.last_sync_at,
        "lastSyncStatus": sync.last_sync_status,
    }))
}

/// Save sync configuration.
#[tauri::command]
pub async fn save_sync_config(
    state: State<'_, AppState>,
    enabled: bool,
    server_url: String,
    username: String,
    password: String,
    remote_dir: String,
    interval_mins: u64,
    sync_config_flag: bool,
    sync_glossary: bool,
    sync_history: bool,
    sync_wordbook: bool,
) -> Result<(), AppError> {
    validate_sync_inputs(&server_url, &remote_dir, interval_mins)?;

    let mut config = state.system.config.lock().await;
    config.sync.enabled = enabled;
    config.sync.server_url = server_url;
    config.sync.username = username;
    // Only update password if a new one is provided (non-empty)
    if !password.is_empty() {
        config.sync.password = password;
    }
    config.sync.remote_dir = remote_dir;
    config.sync.interval_mins = interval_mins;
    config.sync.sync_config = sync_config_flag;
    config.sync.sync_glossary = sync_glossary;
    config.sync.sync_history = sync_history;
    config.sync.sync_wordbook = sync_wordbook;
    config.save();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_sync_inputs_accepts_https_remote_dir() {
        assert!(validate_sync_inputs("https://dav.example.com/dav", "moontranslator", 30).is_ok());
    }

    #[test]
    fn test_validate_sync_inputs_rejects_unsupported_scheme() {
        let err = validate_sync_inputs("file:///tmp/sync", "moontranslator", 30).unwrap_err();

        assert!(err.to_string().contains("http or https"));
    }

    #[test]
    fn test_validate_sync_inputs_rejects_remote_dir_traversal() {
        let err =
            validate_sync_inputs("https://dav.example.com/dav", "../secrets", 30).unwrap_err();

        assert!(err.to_string().contains("remote directory"));
    }

    #[test]
    fn test_validate_sync_inputs_rejects_extreme_interval() {
        let err =
            validate_sync_inputs("https://dav.example.com/dav", "moontranslator", 0).unwrap_err();

        assert!(err.to_string().contains("interval"));
    }
}

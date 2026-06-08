use crate::plugin::{self, PluginInfo, PluginErrorLog, PluginSandboxStatus, MarketplaceEntry};
use crate::plugin_sandbox;
use crate::security;

#[tauri::command]
pub async fn get_plugins() -> Result<Vec<PluginInfo>, String> {
    Ok(plugin::scan_plugins())
}

#[tauri::command]
pub async fn set_plugin_enabled(plugin_name: String, enabled: bool) -> Result<(), String> {
    let _ = security::sanitize_plugin_name(&plugin_name)?;
    plugin::set_plugin_enabled(&plugin_name, enabled)
}

#[tauri::command]
pub async fn get_plugins_dir() -> Result<String, String> {
    Ok(plugin::get_plugins_dir_path())
}

#[tauri::command]
pub async fn open_plugins_dir() -> Result<(), String> {
    let path = plugin::get_plugins_dir_path();
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn install_plugin(source: String) -> Result<PluginInfo, String> {
    plugin::install_plugin(&source).await
}

#[tauri::command]
pub async fn uninstall_plugin(id: String) -> Result<(), String> {
    let _ = security::sanitize_plugin_name(&id)?;
    plugin::uninstall_plugin(&id)
}

#[tauri::command]
pub async fn enable_plugin(id: String) -> Result<(), String> {
    let _ = security::sanitize_plugin_name(&id)?;
    plugin::set_plugin_enabled(&id, true)
}

#[tauri::command]
pub async fn disable_plugin(id: String) -> Result<(), String> {
    let _ = security::sanitize_plugin_name(&id)?;
    plugin::set_plugin_enabled(&id, false)
}

#[tauri::command]
pub async fn check_plugin_update(plugin_name: String) -> Result<PluginUpdateInfo, String> {
    let _ = security::sanitize_plugin_name(&plugin_name)?;
    let (has_update, latest_version) = plugin::check_plugin_update(&plugin_name).await?;
    Ok(PluginUpdateInfo {
        has_update,
        latest_version,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginUpdateInfo {
    pub has_update: bool,
    pub latest_version: String,
}

#[tauri::command]
pub async fn get_plugin_errors(plugin_name: Option<String>) -> Result<Vec<PluginErrorLog>, String> {
    Ok(plugin::get_plugin_errors(plugin_name.as_deref().unwrap_or("")))
}

// ---------------------------------------------------------------------------
// Sandbox commands
// ---------------------------------------------------------------------------

/// Start a plugin in a sandboxed subprocess.
#[tauri::command]
pub async fn start_plugin_sandbox(plugin_name: String) -> Result<(), String> {
    let _ = security::sanitize_plugin_name(&plugin_name)?;
    plugin_sandbox::get_sandbox().start_plugin(&plugin_name).await
}

/// Stop a sandboxed plugin subprocess.
#[tauri::command]
pub async fn stop_plugin_sandbox(plugin_name: String) -> Result<(), String> {
    let _ = security::sanitize_plugin_name(&plugin_name)?;
    plugin_sandbox::get_sandbox().stop_plugin(&plugin_name).await
}

/// Get the sandbox status of a specific plugin.
#[tauri::command]
pub async fn get_plugin_sandbox_status(
    plugin_name: String,
) -> Result<Option<PluginSandboxStatus>, String> {
    let _ = security::sanitize_plugin_name(&plugin_name)?;
    Ok(plugin_sandbox::get_sandbox()
        .get_plugin_status(&plugin_name)
        .await)
}

/// Get the sandbox status of all plugins.
#[tauri::command]
pub async fn get_all_plugin_sandbox_status() -> Result<Vec<PluginSandboxStatus>, String> {
    Ok(plugin_sandbox::get_sandbox().get_all_status().await)
}

/// Send a translation request to a sandboxed plugin.
#[tauri::command]
pub async fn sandbox_translate(
    plugin_name: String,
    text: String,
    from: String,
    to: String,
) -> Result<String, String> {
    let _ = security::sanitize_plugin_name(&plugin_name)?;
    security::validate_language_code(&from)?;
    security::validate_language_code(&to)?;
    plugin_sandbox::get_sandbox()
        .send_translation_request(&plugin_name, &text, &from, &to)
        .await
}

// ---------------------------------------------------------------------------
// Marketplace commands
// ---------------------------------------------------------------------------

/// List all marketplace plugins. Cross-references the local registry with
/// installed plugins to compute the current status of each entry.
#[tauri::command]
pub async fn plugin_list_marketplace() -> Result<Vec<MarketplaceEntry>, String> {
    Ok(plugin::list_marketplace_plugins())
}

/// Install a marketplace plugin by its id.
#[tauri::command]
pub async fn plugin_install_marketplace(id: String) -> Result<MarketplaceEntry, String> {
    plugin::install_marketplace_plugin(&id)
}

/// Uninstall a marketplace plugin by its id.
#[tauri::command]
pub async fn plugin_uninstall_marketplace(id: String) -> Result<(), String> {
    plugin::uninstall_marketplace_plugin(&id)
}

/// Update a marketplace plugin by its id.
#[tauri::command]
pub async fn plugin_update_marketplace(id: String) -> Result<MarketplaceEntry, String> {
    plugin::update_marketplace_plugin(&id)
}

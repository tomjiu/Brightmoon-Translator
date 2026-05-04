use crate::plugin::{self, PluginInfo};

#[tauri::command]
pub async fn get_plugins() -> Result<Vec<PluginInfo>, String> {
    Ok(plugin::scan_plugins())
}

#[tauri::command]
pub async fn set_plugin_enabled(
    plugin_name: String,
    enabled: bool,
) -> Result<(), String> {
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

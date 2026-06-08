use crate::config::HookConfig;
use crate::hook_profile::HookProfileUpdate;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_hook_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<crate::hook_profile::HookProfile>, String> {
    Ok(state.hook.profiles.get_all())
}

#[tauri::command]
pub async fn get_active_hook_profile(
    state: State<'_, AppState>,
) -> Result<Option<crate::hook_profile::HookProfile>, String> {
    Ok(state.hook.profiles.get_active())
}

#[tauri::command]
pub async fn create_hook_profile(
    state: State<'_, AppState>,
    name: String,
    hook_config: HookConfig,
) -> Result<crate::hook_profile::HookProfile, String> {
    Ok(state.hook.profiles.create(name, hook_config))
}

#[tauri::command]
pub async fn update_hook_profile(
    state: State<'_, AppState>,
    id: String,
    updates: HookProfileUpdate,
) -> Result<(), String> {
    state.hook.profiles.update(&id, updates);
    Ok(())
}

#[tauri::command]
pub async fn delete_hook_profile(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state.hook.profiles.delete(&id);
    Ok(())
}

#[tauri::command]
pub async fn activate_hook_profile(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<(), String> {
    state.hook.profiles.activate(id.as_deref());
    Ok(())
}

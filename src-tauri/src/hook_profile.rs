use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

use crate::config::HookConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookProfile {
    pub id: String,
    pub name: String,
    /// Optional: process name to auto-match (e.g., "game.exe")
    pub process_name: Option<String>,
    /// Optional: window title pattern to auto-match
    pub window_title_pattern: Option<String>,
    pub hook_config: HookConfig,
    /// Source language override for this profile
    pub source_lang: Option<String>,
    /// Target language override for this profile
    pub target_lang: Option<String>,
    /// Custom notes
    pub notes: String,
    pub created_at: i64,
    pub last_used: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookProfileStore {
    profiles: Vec<HookProfile>,
    /// ID of the currently active profile (if any)
    active_profile_id: Option<String>,
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!("Failed to create config directory {:?}: {}", path, e);
    }
    path.push("hook_profiles.json");
    path
}

pub struct HookProfileManager {
    store: Mutex<HookProfileStore>,
}

impl HookProfileManager {
    pub fn load() -> Self {
        let path = config_path();
        let empty = HookProfileStore {
            profiles: Vec::new(),
            active_profile_id: None,
        };
        let store = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
                    tracing::error!("Failed to parse hook profiles {:?}: {}", path, e);
                    empty
                }),
                Err(e) => {
                    tracing::error!("Failed to read hook profiles {:?}: {}", path, e);
                    empty
                },
            }
        } else {
            empty
        };

        Self {
            store: Mutex::new(store),
        }
    }

    fn save(&self) {
        let store = self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let path = config_path();
        match serde_json::to_string_pretty(&*store) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    tracing::error!("Failed to save hook profiles {:?}: {}", path, e);
                }
            },
            Err(e) => {
                tracing::error!("Failed to serialize hook profiles: {}", e);
            },
        }
    }

    pub fn get_all(&self) -> Vec<HookProfile> {
        self.store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .profiles
            .clone()
    }

    pub fn get_active(&self) -> Option<HookProfile> {
        let store = self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(ref id) = store.active_profile_id {
            store.profiles.iter().find(|p| &p.id == id).cloned()
        } else {
            None
        }
    }

    pub fn get_active_id(&self) -> Option<String> {
        self.store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active_profile_id
            .clone()
    }

    pub fn create(&self, name: String, hook_config: HookConfig) -> HookProfile {
        let profile = HookProfile {
            id: Uuid::new_v4().to_string(),
            name,
            process_name: None,
            window_title_pattern: None,
            hook_config,
            source_lang: None,
            target_lang: None,
            notes: String::new(),
            created_at: chrono::Utc::now().timestamp(),
            last_used: None,
        };

        let mut store = self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        store.profiles.push(profile.clone());
        drop(store);
        self.save();

        profile
    }

    pub fn update(&self, id: &str, updates: HookProfileUpdate) {
        let mut store = self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(profile) = store.profiles.iter_mut().find(|p| p.id == id) {
            if let Some(name) = updates.name {
                profile.name = name;
            }
            if let Some(process_name) = updates.process_name {
                profile.process_name = Some(process_name);
            }
            if let Some(window_title_pattern) = updates.window_title_pattern {
                profile.window_title_pattern = Some(window_title_pattern);
            }
            if let Some(hook_config) = updates.hook_config {
                profile.hook_config = hook_config;
            }
            if let Some(source_lang) = updates.source_lang {
                profile.source_lang = Some(source_lang);
            }
            if let Some(target_lang) = updates.target_lang {
                profile.target_lang = Some(target_lang);
            }
            if let Some(notes) = updates.notes {
                profile.notes = notes;
            }
        }
        drop(store);
        self.save();
    }

    pub fn delete(&self, id: &str) {
        let mut store = self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        store.profiles.retain(|p| p.id != id);
        if store.active_profile_id.as_deref() == Some(id) {
            store.active_profile_id = None;
        }
        drop(store);
        self.save();
    }

    pub fn activate(&self, id: Option<&str>) {
        let mut store = self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        store.active_profile_id = id.map(std::string::ToString::to_string);

        // Update last_used timestamp
        if let Some(id) = id {
            if let Some(profile) = store.profiles.iter_mut().find(|p| p.id == id) {
                profile.last_used = Some(chrono::Utc::now().timestamp());
            }
        }

        drop(store);
        self.save();
    }

    /// Find a profile that matches the given process name or window title
    pub fn auto_match(&self, process_name: &str, window_title: &str) -> Option<HookProfile> {
        let store = self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);

        // First try to match by process name
        for profile in &store.profiles {
            if let Some(ref pattern) = profile.process_name {
                if process_name
                    .to_lowercase()
                    .contains(&pattern.to_lowercase())
                {
                    return Some(profile.clone());
                }
            }
        }

        // Then try to match by window title pattern
        for profile in &store.profiles {
            if let Some(ref pattern) = profile.window_title_pattern {
                if window_title
                    .to_lowercase()
                    .contains(&pattern.to_lowercase())
                {
                    return Some(profile.clone());
                }
            }
        }

        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookProfileUpdate {
    pub name: Option<String>,
    pub process_name: Option<String>,
    pub window_title_pattern: Option<String>,
    pub hook_config: Option<HookConfig>,
    pub source_lang: Option<String>,
    pub target_lang: Option<String>,
    pub notes: Option<String>,
}

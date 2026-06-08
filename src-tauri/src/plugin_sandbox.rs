use crate::plugin::{self, PluginInfo, PluginPermission, PluginRunState, PluginSandboxStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};

// ---------------------------------------------------------------------------
// IPC Protocol
// ---------------------------------------------------------------------------

/// Messages sent from host to plugin subprocess.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum HostToPlugin {
    /// Initialize the plugin with its configuration.
    Init {
        plugin_name: String,
        plugin_dir: String,
        permissions: Vec<String>,
    },
    /// Request a translation.
    Translate {
        request_id: String,
        text: String,
        from: String,
        to: String,
    },
    /// Ping to check liveness.
    Ping { request_id: String },
    /// Gracefully shut down.
    Shutdown,
}

/// Messages sent from plugin subprocess to host.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum PluginToHost {
    /// Plugin has initialized successfully.
    InitOk,
    /// Translation result.
    TranslateResult {
        request_id: String,
        result: Result<String, String>,
    },
    /// Pong response to ping.
    Pong { request_id: String },
    /// Plugin is reporting an error.
    Error {
        request_id: Option<String>,
        message: String,
    },
    /// Plugin requesting permission check.
    CheckPermission {
        request_id: String,
        permission: String,
    },
}

// ---------------------------------------------------------------------------
// Managed Plugin Instance
// ---------------------------------------------------------------------------

struct ManagedPlugin {
    info: PluginInfo,
    child: Option<Child>,
    state: PluginRunState,
    restart_count: u32,
    started_at: Option<Instant>,
    last_health_check: Option<Instant>,
    stdin_tx: Option<tokio::sync::mpsc::Sender<String>>,
    response_map: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PluginToHost>>>>,
}

impl ManagedPlugin {
    fn new(info: PluginInfo) -> Self {
        Self {
            info,
            child: None,
            state: PluginRunState::Stopped,
            restart_count: 0,
            started_at: None,
            last_health_check: None,
            stdin_tx: None,
            response_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(|c| c.id())
    }

    fn uptime_ms(&self) -> u64 {
        self.started_at
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Helper to get the response request_id from a PluginToHost message.
#[allow(dead_code)]
fn get_request_id(msg: &PluginToHost) -> Option<&str> {
    match msg {
        PluginToHost::TranslateResult { request_id, .. } => Some(request_id.as_str()),
        PluginToHost::Pong { request_id } => Some(request_id.as_str()),
        _ => None,
    }
}

fn resolve_entry_point(plugin_dir: &Path, entry_point: &str) -> Result<PathBuf, String> {
    if entry_point.trim().is_empty() {
        return Err("Plugin entry point is empty".to_string());
    }

    let entry_path = Path::new(entry_point);
    for component in entry_path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {},
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("Plugin entry point must stay inside plugin directory".to_string());
            },
        }
    }

    Ok(plugin_dir.join(entry_path))
}

// ---------------------------------------------------------------------------
// Plugin Sandbox Manager
// ---------------------------------------------------------------------------

/// Manages sandboxed plugin processes with isolation and resource limits.
pub struct PluginSandbox {
    plugins: Arc<RwLock<HashMap<String, ManagedPlugin>>>,
}

impl PluginSandbox {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a plugin in a sandboxed subprocess.
    pub async fn start_plugin(&self, plugin_name: &str) -> Result<(), String> {
        let all_plugins = plugin::scan_plugins();
        let plugin_info = all_plugins
            .iter()
            .find(|p| p.manifest.name == plugin_name)
            .ok_or_else(|| format!("Plugin '{}' not found", plugin_name))?
            .clone();

        if !plugin_info.manifest.sandbox.enabled {
            return Err(format!(
                "Plugin '{}' does not have sandbox enabled",
                plugin_name
            ));
        }

        if plugin_info.manifest.entry_point.is_empty() {
            return Err(format!(
                "Plugin '{}' has no entry_point defined in manifest",
                plugin_name
            ));
        }

        let mut plugins = self.plugins.write().await;

        // Stop existing instance if running
        if let Some(existing) = plugins.get_mut(plugin_name) {
            if existing.state == PluginRunState::Running {
                Self::stop_managed(existing).await;
            }
        }

        let mut managed = ManagedPlugin::new(plugin_info.clone());
        Self::spawn_process(&mut managed, &plugin_info).await?;

        let state_str = format!("{:?}", managed.state);
        plugins.insert(plugin_name.to_string(), managed);

        tracing::info!(
            "Plugin '{}' started in sandbox, state={}",
            plugin_name,
            state_str
        );
        Ok(())
    }

    /// Stop a running plugin subprocess.
    pub async fn stop_plugin(&self, plugin_name: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write().await;
        if let Some(managed) = plugins.get_mut(plugin_name) {
            Self::stop_managed(managed).await;
            Ok(())
        } else {
            Err(format!("Plugin '{}' is not running", plugin_name))
        }
    }

    /// Send a translation request to a sandboxed plugin and wait for response.
    pub async fn send_translation_request(
        &self,
        plugin_name: &str,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, String> {
        let plugins = self.plugins.read().await;
        let managed = plugins
            .get(plugin_name)
            .ok_or_else(|| format!("Plugin '{}' is not running", plugin_name))?;

        if managed.state != PluginRunState::Running {
            return Err(format!(
                "Plugin '{}' is not in running state (current: {:?})",
                plugin_name, managed.state
            ));
        }

        // Check Network permission
        Self::check_permission_static(&managed.info, &PluginPermission::Network)?;

        let request_id = uuid::Uuid::new_v4().to_string();
        let msg = HostToPlugin::Translate {
            request_id: request_id.clone(),
            text: text.to_string(),
            from: from.to_string(),
            to: to.to_string(),
        };

        let (tx, rx) = tokio::sync::oneshot::channel();
        managed
            .response_map
            .lock()
            .await
            .insert(request_id.clone(), tx);

        let json =
            serde_json::to_string(&msg).map_err(|e| format!("Serialization error: {}", e))?;

        if let Some(stdin_tx) = &managed.stdin_tx {
            stdin_tx
                .send(json)
                .await
                .map_err(|e| format!("Failed to send to plugin stdin: {}", e))?;
        } else {
            return Err("Plugin stdin channel not available".to_string());
        }

        // Wait for response with timeout
        let timeout = Duration::from_secs(30);
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(PluginToHost::TranslateResult { result, .. })) => result,
            Ok(Ok(other)) => Err(format!("Unexpected response: {:?}", other)),
            Ok(Err(_)) => Err("Response channel closed".to_string()),
            Err(_) => {
                // Clean up pending response
                managed.response_map.lock().await.remove(&request_id);
                Err("Plugin response timeout".to_string())
            },
        }
    }

    fn check_permission_static(
        plugin_info: &PluginInfo,
        permission: &PluginPermission,
    ) -> Result<(), String> {
        if plugin_info.manifest.permissions.contains(permission) {
            Ok(())
        } else {
            Err(format!(
                "Plugin '{}' does not have '{}' permission. Required permissions: {:?}",
                plugin_info.manifest.name,
                serde_json::to_string(permission).unwrap_or_default(),
                plugin_info
                    .manifest
                    .permissions
                    .iter()
                    .map(|p| serde_json::to_string(p).unwrap_or_default())
                    .collect::<Vec<_>>()
            ))
        }
    }

    /// Get status of a specific plugin sandbox.
    pub async fn get_plugin_status(&self, plugin_name: &str) -> Option<PluginSandboxStatus> {
        let plugins = self.plugins.read().await;
        plugins.get(plugin_name).map(|m| {
            let (mem, cpu) = Self::get_process_resource_usage(m.pid());
            PluginSandboxStatus {
                plugin_name: plugin_name.to_string(),
                pid: m.pid(),
                state: m.state.clone(),
                memory_usage_mb: mem,
                cpu_usage_percent: cpu,
                restart_count: m.restart_count,
                uptime_ms: m.uptime_ms(),
            }
        })
    }

    /// Get status of all managed plugin sandboxes.
    pub async fn get_all_status(&self) -> Vec<PluginSandboxStatus> {
        let plugins = self.plugins.read().await;
        plugins
            .iter()
            .map(|(name, m)| {
                let (mem, cpu) = Self::get_process_resource_usage(m.pid());
                PluginSandboxStatus {
                    plugin_name: name.clone(),
                    pid: m.pid(),
                    state: m.state.clone(),
                    memory_usage_mb: mem,
                    cpu_usage_percent: cpu,
                    restart_count: m.restart_count,
                    uptime_ms: m.uptime_ms(),
                }
            })
            .collect()
    }

    /// Run health checks on all running plugins. Should be called periodically.
    pub async fn health_check_all(&self) {
        let mut plugins = self.plugins.write().await;
        let mut to_restart = Vec::new();

        // First pass: check which plugins need attention
        for (name, managed) in plugins.iter_mut() {
            if managed.state != PluginRunState::Running {
                continue;
            }

            // Check if process is still alive
            if let Some(child) = &mut managed.child {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        tracing::warn!("Plugin '{}' exited with status: {:?}", name, status);
                        managed.state = PluginRunState::Crashed;
                        plugin::log_plugin_error(
                            name,
                            &format!("Plugin process exited with status: {:?}", status),
                        );
                        to_restart.push(name.clone());
                    },
                    Ok(None) => {
                        // Still running
                        managed.last_health_check = Some(Instant::now());
                        // Note: Memory monitoring would require Windows Job Objects
                        // or periodic polling. For now we just check liveness.
                    },
                    Err(e) => {
                        tracing::error!("Error checking plugin '{}' status: {}", name, e);
                    },
                }
            }
        }

        // Second pass: restart crashed plugins
        for name in to_restart {
            if let Some(managed) = plugins.get_mut(&name) {
                let max_restarts = managed.info.manifest.sandbox.max_restarts;
                if managed.restart_count < max_restarts {
                    managed.restart_count += 1;
                    managed.state = PluginRunState::Restarting;
                    let info = managed.info.clone();
                    tracing::info!(
                        "Restarting plugin '{}' (attempt {}/{})",
                        name,
                        managed.restart_count,
                        max_restarts
                    );
                    if let Err(e) = Self::spawn_process(managed, &info).await {
                        tracing::error!("Failed to restart plugin '{}': {}", name, e);
                        managed.state = PluginRunState::Crashed;
                        plugin::log_plugin_error(&name, &format!("Restart failed: {}", e));
                    }
                } else {
                    tracing::error!(
                        "Plugin '{}' exceeded max restarts ({}), giving up",
                        name,
                        max_restarts
                    );
                    managed.state = PluginRunState::Crashed;
                    plugin::log_plugin_error(
                        &name,
                        &format!("Exceeded max restarts ({})", max_restarts),
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    async fn spawn_process(managed: &mut ManagedPlugin, info: &PluginInfo) -> Result<(), String> {
        let plugin_dir = PathBuf::from(&info.path);
        let entry_point = resolve_entry_point(&plugin_dir, &info.manifest.entry_point)?;

        if !entry_point.exists() {
            return Err(format!(
                "Plugin entry point not found: {}",
                entry_point.display()
            ));
        }

        let sandbox_cfg = &info.manifest.sandbox;

        let mut cmd = Command::new(&entry_point);
        cmd.current_dir(&plugin_dir);

        // Set environment variables for the plugin
        cmd.env("MOON_PLUGIN_NAME", &info.manifest.name);
        cmd.env("MOON_PLUGIN_DIR", plugin_dir.to_string_lossy().to_string());
        cmd.env(
            "MOON_PLUGIN_PERMISSIONS",
            serde_json::to_string(
                &info
                    .manifest
                    .permissions
                    .iter()
                    .map(|p| serde_json::to_string(p).unwrap_or_default())
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default(),
        );
        cmd.env(
            "MOON_PLUGIN_MAX_MEMORY_MB",
            sandbox_cfg.max_memory_mb.to_string(),
        );
        cmd.env(
            "MOON_PLUGIN_MAX_CPU_PERCENT",
            sandbox_cfg.max_cpu_percent.to_string(),
        );

        // Configure stdin/stdout for IPC
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            // CREATE_NO_WINDOW - prevents console window from appearing
            cmd.creation_flags(0x08000000);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn plugin process: {}", e))?;

        let pid = child.id().unwrap_or(0);
        tracing::info!("Plugin '{}' spawned with PID {}", info.manifest.name, pid);

        // Note: Resource limits are communicated via environment variables.
        // The plugin process is expected to respect MOON_PLUGIN_MAX_MEMORY_MB
        // and MOON_PLUGIN_MAX_CPU_PERCENT. On Windows, we could use Job Objects
        // for enforcement, but that requires additional feature flags in the
        // windows crate. For now, we rely on cooperative resource management.

        // Set up IPC channels
        let stdin = child.stdin.take().ok_or("Failed to open plugin stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to open plugin stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to open plugin stderr")?;

        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel::<String>(64);
        let response_map = managed.response_map.clone();

        // Spawn stdin writer task
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(msg) = stdin_rx.recv().await {
                let line = format!("{}\n", msg);
                if let Err(e) = stdin.write_all(line.as_bytes()).await {
                    tracing::error!("Failed to write to plugin stdin: {}", e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    tracing::error!("Failed to flush plugin stdin: {}", e);
                    break;
                }
            }
        });

        // Spawn stdout reader task
        let response_map_clone = response_map.clone();
        let plugin_name = info.manifest.name.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<PluginToHost>(&line) {
                    Ok(msg) => {
                        match &msg {
                            PluginToHost::InitOk => {
                                tracing::info!("Plugin '{}' initialized", plugin_name);
                            },
                            PluginToHost::Pong { request_id } => {
                                tracing::debug!("Plugin '{}' pong: {}", plugin_name, request_id);
                            },
                            PluginToHost::Error { message, .. } => {
                                tracing::error!("Plugin '{}' error: {}", plugin_name, message);
                                plugin::log_plugin_error(&plugin_name, message);
                            },
                            PluginToHost::CheckPermission {
                                request_id,
                                permission,
                            } => {
                                tracing::info!(
                                    "Plugin '{}' requesting permission: {} (id: {})",
                                    plugin_name,
                                    permission,
                                    request_id
                                );
                                // Permission check is handled by the host
                            },
                            _ => {},
                        }

                        // Try to deliver to waiting request
                        if let Some(req_id) = get_request_id(&msg) {
                            let mut map = response_map_clone.lock().await;
                            if let Some(tx) = map.remove(req_id) {
                                let _ = tx.send(msg);
                            }
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            "Plugin '{}' sent invalid message: {} (raw: {})",
                            plugin_name,
                            e,
                            line
                        );
                    },
                }
            }
            tracing::info!("Plugin '{}' stdout reader exited", plugin_name);
        });

        // Spawn stderr reader task (for logging)
        let plugin_name_err = info.manifest.name.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!("[plugin:{}] stderr: {}", plugin_name_err, line);
            }
        });

        // Send init message
        let init_msg = HostToPlugin::Init {
            plugin_name: info.manifest.name.clone(),
            plugin_dir: plugin_dir.to_string_lossy().to_string(),
            permissions: info
                .manifest
                .permissions
                .iter()
                .map(|p| serde_json::to_string(p).unwrap_or_default())
                .collect(),
        };
        let init_json =
            serde_json::to_string(&init_msg).map_err(|e| format!("Init serialization: {}", e))?;
        if let Err(e) = stdin_tx.send(init_json).await {
            return Err(format!("Failed to send init message: {}", e));
        }

        managed.child = Some(child);
        managed.state = PluginRunState::Running;
        managed.started_at = Some(Instant::now());
        managed.stdin_tx = Some(stdin_tx);

        Ok(())
    }

    async fn stop_managed(managed: &mut ManagedPlugin) {
        // Try graceful shutdown first
        if let Some(stdin_tx) = &managed.stdin_tx {
            let shutdown = HostToPlugin::Shutdown;
            if let Ok(json) = serde_json::to_string(&shutdown) {
                let _ = stdin_tx.send(json).await;
            }
        }

        // Wait a bit for graceful exit
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Force kill if still running
        if let Some(child) = &mut managed.child {
            if child.try_wait().ok().flatten().is_none() {
                tracing::warn!("Force killing plugin '{}'", managed.info.manifest.name);
                let _ = child.kill().await;
            }
        }

        managed.child = None;
        managed.state = PluginRunState::Stopped;
        managed.stdin_tx = None;
    }

    /// Get process resource usage (memory in MB, CPU in percent).
    /// Returns (0, 0) if unable to query.
    /// Note: Full resource monitoring requires platform-specific APIs.
    /// Currently returns placeholder values; plugins self-report via IPC.
    fn get_process_resource_usage(_pid: Option<u32>) -> (u64, f64) {
        // Resource monitoring placeholder
        // Plugins can report their own usage via the IPC protocol
        (0, 0.0)
    }
}

// ---------------------------------------------------------------------------
// Singleton access
// ---------------------------------------------------------------------------

static PLUGIN_SANDBOX: std::sync::OnceLock<PluginSandbox> = std::sync::OnceLock::new();

/// Get the global plugin sandbox instance.
pub fn get_sandbox() -> &'static PluginSandbox {
    PLUGIN_SANDBOX.get_or_init(PluginSandbox::new)
}

/// Initialize the plugin sandbox (should be called at app startup).
pub fn init_sandbox() -> &'static PluginSandbox {
    let sandbox = PluginSandbox::new();
    let _ = PLUGIN_SANDBOX.set(sandbox);
    get_sandbox()
}

// ---------------------------------------------------------------------------
// Background health check task
// ---------------------------------------------------------------------------

/// Spawn a background task that periodically checks plugin health.
pub fn spawn_health_check_task() {
    tauri::async_runtime::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            get_sandbox().health_check_all().await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_entry_point_rejects_parent_traversal() {
        let plugin_dir = PathBuf::from("C:\\plugins\\demo");

        let err = resolve_entry_point(&plugin_dir, "..\\other.exe").unwrap_err();

        assert!(err.contains("must stay inside plugin directory"));
    }

    #[test]
    fn test_resolve_entry_point_accepts_relative_child() {
        let plugin_dir = PathBuf::from("C:\\plugins\\demo");

        let resolved = resolve_entry_point(&plugin_dir, "bin\\plugin.exe").unwrap();

        assert_eq!(resolved, plugin_dir.join("bin\\plugin.exe"));
    }
}

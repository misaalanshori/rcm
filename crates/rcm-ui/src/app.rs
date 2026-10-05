use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio::sync::RwLock;
use rcm_core::DaemonState;
use rcm_ipc::protocol::RcConnectionInfo;
use rcm_ipc::IpcClient;
use rcm_rc::RcClient;
use rcm_ui_kit::view_model::{
    CommandPaletteItem, DashboardViewModel, FileEntryViewModel, MountItemViewModel,
    RemoteItemViewModel, ServeItemViewModel, SidebarDestination,
};
use rcm_ui_kit::ThemeTokens;

pub struct AppState {
    pub current_destination: SidebarDestination,
    pub theme: ThemeTokens,
    pub daemon_state: DaemonState,
    pub rc_client: Option<RcClient>,
    pub ipc_client: Option<IpcClient>,
    pub remotes: Vec<RemoteItemViewModel>,
    pub mounts: Vec<MountItemViewModel>,
    pub serves: Vec<ServeItemViewModel>,
    pub files: Vec<FileEntryViewModel>,
    pub recent_errors: Vec<String>,
    pub command_palette_open: bool,
    pub command_palette_query: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_destination: SidebarDestination::Home,
            theme: ThemeTokens::dark(),
            daemon_state: DaemonState::Stopped,
            rc_client: None,
            ipc_client: None,
            remotes: Vec::new(),
            mounts: Vec::new(),
            serves: Vec::new(),
            files: Vec::new(),
            recent_errors: Vec::new(),
            command_palette_open: false,
            command_palette_query: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct AppController {
    state: Arc<RwLock<AppState>>,
}

impl AppController {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(AppState::default())),
        }
    }

    pub fn state(&self) -> Arc<RwLock<AppState>> {
        self.state.clone()
    }

    pub async fn switch_destination(&self, dest: SidebarDestination) {
        let mut s = self.state.write().await;
        s.current_destination = dest;
    }

    pub async fn toggle_command_palette(&self) {
        let mut s = self.state.write().await;
        s.command_palette_open = !s.command_palette_open;
        if !s.command_palette_open {
            s.command_palette_query.clear();
        }
    }

    pub async fn set_command_palette_query(&self, query: impl Into<String>) {
        let mut s = self.state.write().await;
        s.command_palette_query = query.into();
    }

    pub async fn get_dashboard_view_model(&self) -> DashboardViewModel {
        let s = self.state.read().await;
        let active_mounts = s.mounts.iter().filter(|m| m.is_mounted).count();
        let active_serves = s.serves.iter().filter(|sv| sv.is_running).count();

        DashboardViewModel {
            daemon_state: s.daemon_state.clone(),
            active_mounts_count: active_mounts,
            active_serves_count: active_serves,
            running_jobs_count: 0,
            recent_errors: s.recent_errors.clone(),
        }
    }

    pub async fn get_command_palette_items(&self) -> Vec<CommandPaletteItem> {
        let s = self.state.read().await;
        let query = s.command_palette_query.to_lowercase();

        let base_items = vec![
            CommandPaletteItem {
                title: "Go to Dashboard".to_string(),
                subtitle: Some("Overview of remotes and daemon".to_string()),
                shortcut: Some("1".to_string()),
                destination: Some(SidebarDestination::Home),
                action_id: "nav.home".to_string(),
            },
            CommandPaletteItem {
                title: "Go to Remotes".to_string(),
                subtitle: Some("Manage cloud providers and backends".to_string()),
                shortcut: Some("2".to_string()),
                destination: Some(SidebarDestination::Remotes),
                action_id: "nav.remotes".to_string(),
            },
            CommandPaletteItem {
                title: "Go to Mounts".to_string(),
                subtitle: Some("Drive letters and VFS profiles".to_string()),
                shortcut: Some("3".to_string()),
                destination: Some(SidebarDestination::Mounts),
                action_id: "nav.mounts".to_string(),
            },
            CommandPaletteItem {
                title: "Go to Serves".to_string(),
                subtitle: Some("WebDAV, SFTP, HTTP and S3 endpoints".to_string()),
                shortcut: Some("4".to_string()),
                destination: Some(SidebarDestination::Serves),
                action_id: "nav.serves".to_string(),
            },
            CommandPaletteItem {
                title: "Go to Files".to_string(),
                subtitle: Some("Remote explorer and file transfers".to_string()),
                shortcut: Some("5".to_string()),
                destination: Some(SidebarDestination::Files),
                action_id: "nav.files".to_string(),
            },
            CommandPaletteItem {
                title: "Go to Settings".to_string(),
                subtitle: Some("Daemon supervision, backups and logs".to_string()),
                shortcut: Some("6".to_string()),
                destination: Some(SidebarDestination::Settings),
                action_id: "nav.settings".to_string(),
            },
        ];

        if query.is_empty() {
            base_items
        } else {
            base_items
                .into_iter()
                .filter(|item| {
                    item.title.to_lowercase().contains(&query)
                        || item
                            .subtitle
                            .as_ref()
                            .map(|s| s.to_lowercase().contains(&query))
                            .unwrap_or(false)
                })
                .collect()
        }
    }

    pub async fn connect_to_agent(&self, pipe_name: &str) -> Result<(), rcm_core::CoreError> {
        let ipc = IpcClient::connect(pipe_name).await?;

        // Retrieve daemon status
        let status_val = ipc.call("daemon.status", serde_json::json!({})).await?;
        let state: DaemonState = serde_json::from_value(status_val).unwrap_or(DaemonState::Stopped);

        // Retrieve direct RC connection info if available
        let rc_client = if state.is_ready() {
            if let Ok(conn_val) = ipc.call("rc.connection_info", serde_json::json!({})).await {
                if let Ok(info) = serde_json::from_value::<RcConnectionInfo>(conn_val) {
                    Some(RcClient::new(
                        info.addr,
                        info.auth_user.as_deref(),
                        info.auth_pass.as_deref(),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut s = self.state.write().await;
        s.daemon_state = state;
        s.ipc_client = Some(ipc);
        s.rc_client = rc_client;

        Ok(())
    }

    /// Automatically ensures rcm-agent is running (spawning it in background if needed)
    /// and that rclone daemon is installed and started without requiring manual steps (UX zero-friction)
    pub async fn ensure_connected_and_ready(&self, pipe_name: &str) -> Result<(), rcm_core::CoreError> {
        // Step 1: Connect to agent or auto-spawn if not running
        let ipc = match IpcClient::connect(pipe_name).await {
            Ok(c) => c,
            Err(_) => {
                // Agent not running: attempt to auto-spawn in background
                let agent_exe = find_agent_binary();
                if let Some(exe_path) = agent_exe {
                    let mut cmd = Command::new(&exe_path);
                    cmd.arg("--background").arg("--pipe").arg(pipe_name);

                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                    }

                    let _ = cmd.spawn();

                    // Retry connecting
                    let mut connected = None;
                    for _ in 0..35 {
                        sleep(Duration::from_millis(100)).await;
                        if let Ok(c) = IpcClient::connect(pipe_name).await {
                            connected = Some(c);
                            break;
                        }
                    }

                    match connected {
                        Some(c) => c,
                        None => {
                            return Err(rcm_core::CoreError::Connection(format!(
                                "Failed to connect to rcm-agent after spawning '{}'",
                                exe_path
                            )));
                        }
                    }
                } else {
                    return Err(rcm_core::CoreError::NotFound(
                        "rcm-agent executable could not be located to auto-start".to_string(),
                    ));
                }
            }
        };

        // Step 2: Ensure daemon is running
        let status_val = ipc.call("daemon.status", serde_json::json!({})).await?;
        let mut state: DaemonState = serde_json::from_value(status_val).unwrap_or(DaemonState::Stopped);

        if !state.is_ready() {
            // Try starting daemon
            let start_res = ipc.call("daemon.start", serde_json::json!({})).await;
            match start_res {
                Ok(_) => {
                    sleep(Duration::from_millis(300)).await;
                    let new_status = ipc.call("daemon.status", serde_json::json!({})).await?;
                    state = serde_json::from_value(new_status).unwrap_or(DaemonState::Stopped);
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    if err_msg.contains("No registered rclone binary found") {
                        // Auto-fetch latest rclone binary!
                        println!("First run setup: automatically downloading official rclone release...");
                        let _ = ipc.call("binary.fetch", serde_json::Value::Null).await?;
                        let _ = ipc.call("daemon.start", serde_json::json!({})).await?;
                        sleep(Duration::from_millis(300)).await;
                        let new_status = ipc.call("daemon.status", serde_json::json!({})).await?;
                        state = serde_json::from_value(new_status).unwrap_or(DaemonState::Stopped);
                    } else {
                        return Err(rcm_core::CoreError::Validation(format!(
                            "Failed to start rclone daemon: {}",
                            err
                        )));
                    }
                }
            }
        }

        // Step 3: Setup direct RC connection if ready
        let rc_client = if state.is_ready() {
            if let Ok(conn_val) = ipc.call("rc.connection_info", serde_json::json!({})).await {
                if let Ok(info) = serde_json::from_value::<RcConnectionInfo>(conn_val) {
                    Some(RcClient::new(
                        info.addr,
                        info.auth_user.as_deref(),
                        info.auth_pass.as_deref(),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut s = self.state.write().await;
        s.daemon_state = state;
        s.ipc_client = Some(ipc);
        s.rc_client = rc_client;

        Ok(())
    }
}

fn find_agent_binary() -> Option<String> {
    // 1. Next to current executable
    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            let exe_name = if cfg!(windows) { "rcm-agent.exe" } else { "rcm-agent" };
            let candidate = dir.join(exe_name);
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    // 2. Program files / local app data install location
    #[cfg(windows)]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let candidate = std::path::Path::new(&local_app_data)
                .join("Programs")
                .join("RCM")
                .join("rcm-agent.exe");
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let candidate = std::path::Path::new(&home)
                .join(".local")
                .join("bin")
                .join("rcm-agent");
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

impl Default for AppController {
    fn default() -> Self {
        Self::new()
    }
}

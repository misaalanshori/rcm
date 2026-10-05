use std::sync::Arc;
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
}

impl Default for AppController {
    fn default() -> Self {
        Self::new()
    }
}

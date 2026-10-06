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
    tokio_handle: tokio::runtime::Handle,
}

impl AppController {
    pub fn new() -> Self {
        let tokio_handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => {
                // Initialize dedicated multi-thread Tokio runtime per SRDD §7.11
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to initialize background Tokio runtime for RCM");
                let handle = rt.handle().clone();
                // Persist the runtime in background for the lifetime of the UI process
                Box::leak(Box::new(rt));
                handle
            }
        };

        Self {
            state: Arc::new(RwLock::new(AppState::default())),
            tokio_handle,
        }
    }

    pub fn tokio_handle(&self) -> &tokio::runtime::Handle {
        &self.tokio_handle
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
        let _guard = self.tokio_handle.enter();
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
        let app = self.clone();
        let pipe = pipe_name.to_string();
        self.tokio_handle
            .spawn(async move {
                app.ensure_connected_and_ready_internal(&pipe).await
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Task execution failed: {}", e)))?
    }

    async fn ensure_connected_and_ready_internal(&self, pipe_name: &str) -> Result<(), rcm_core::CoreError> {
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

    /// Fetches all latest UI data from IPC agent and direct RC connection
    pub async fn fetch_ui_data(
        &self,
    ) -> (
        DaemonState,
        Vec<RemoteItemViewModel>,
        Vec<MountItemViewModel>,
        Vec<ServeItemViewModel>,
    ) {
        let app = self.clone();
        self.tokio_handle
            .spawn(async move {
                app.fetch_ui_data_internal().await
            })
            .await
            .unwrap_or_else(|_| (DaemonState::Stopped, Vec::new(), Vec::new(), Vec::new()))
    }

    async fn fetch_ui_data_internal(
        &self,
    ) -> (
        DaemonState,
        Vec<RemoteItemViewModel>,
        Vec<MountItemViewModel>,
        Vec<ServeItemViewModel>,
    ) {
        let state_arc = self.state();
        let mut daemon_state = DaemonState::Stopped;
        let mut remotes_list = Vec::new();
        let mut mounts_list = Vec::new();
        let mut serves_list = Vec::new();

        let (ipc_opt, mut rc_opt) = {
            let s = state_arc.read().await;
            (s.ipc_client.clone(), s.rc_client.clone())
        };

        if let Some(ref ipc) = ipc_opt {
            if let Ok(st_val) = ipc.call("daemon.status", serde_json::json!({})).await {
                if let Ok(st) = serde_json::from_value::<DaemonState>(st_val) {
                    daemon_state = st.clone();
                    if st.is_ready() && rc_opt.is_none() {
                        if let Ok(conn_val) = ipc.call("rc.connection_info", serde_json::json!({})).await {
                            if let Ok(info) = serde_json::from_value::<RcConnectionInfo>(conn_val) {
                                let new_rc = RcClient::new(
                                    info.addr,
                                    info.auth_user.as_deref(),
                                    info.auth_pass.as_deref(),
                                );
                                rc_opt = Some(new_rc.clone());
                                state_arc.write().await.rc_client = Some(new_rc);
                            }
                        }
                    }
                }
            }

            // Mount profiles
            if let Ok(m_val) = ipc.call("profiles.list_mounts", serde_json::json!({})).await {
                if let Ok(profiles) = serde_json::from_value::<Vec<rcm_core::MountProfile>>(m_val) {
                    for p in profiles {
                        let target_str = match &p.target {
                            rcm_core::MountTarget::DriveLetter(c) => format!("{}:", c),
                            rcm_core::MountTarget::AutoDriveLetter => "Auto (*)".to_string(),
                            rcm_core::MountTarget::Folder(path) => path.to_string(),
                            rcm_core::MountTarget::Unc(u) => u.clone(),
                        };
                        mounts_list.push(MountItemViewModel {
                            id: p.id,
                            name: p.name,
                            remote: p.remote,
                            target: target_str,
                            preset: format!("{:?}", p.preset),
                            is_mounted: false,
                            cache_used_bytes: None,
                            upload_queue_count: 0,
                        });
                    }
                }
            }

            // Serve profiles
            if let Ok(s_val) = ipc.call("profiles.list_serves", serde_json::json!({})).await {
                if let Ok(profiles) = serde_json::from_value::<Vec<rcm_core::ServeProfile>>(s_val) {
                    for p in profiles {
                        serves_list.push(ServeItemViewModel {
                            id: p.id,
                            name: p.name,
                            remote: p.remote,
                            protocol: p.protocol.to_string().to_uppercase(),
                            addr: p.addr.clone(),
                            is_running: false,
                            url: format!("http://{}", p.addr),
                        });
                    }
                }
            }
        }

        // Remotes from rcd
        if let Some(ref rc) = rc_opt {
            if let Ok(remotes) = rc.config_list_remotes().await {
                for r in remotes {
                    remotes_list.push(RemoteItemViewModel {
                        name: r,
                        backend_type: "cloud".to_string(),
                        referrers: Vec::new(),
                        is_env_defined: false,
                        is_encrypted: false,
                    });
                }
            }

            // Check active running mounts to mark is_mounted
            if let Ok(active_mounts) = rc.mount_list_mounts().await {
                for m in &mut mounts_list {
                    if active_mounts.iter().any(|act| {
                        act.mount_point.trim_end_matches('\\').eq_ignore_ascii_case(m.target.trim_end_matches('\\'))
                    }) {
                        m.is_mounted = true;
                    }
                }
            }
        }

        (daemon_state, remotes_list, mounts_list, serves_list)
    }

    pub async fn trigger_reconcile(&self) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref ipc) = s.ipc_client {
                    let _ = ipc.call("reconcile.now", serde_json::json!({})).await;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Task execution failed: {}", e)))?
    }

    pub async fn trigger_restart_daemon(&self) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref ipc) = s.ipc_client {
                    let _ = ipc.call("daemon.restart", serde_json::json!({})).await;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Task execution failed: {}", e)))?
    }

    pub async fn save_mount_profile(&self, profile: rcm_core::MountProfile) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref ipc) = s.ipc_client {
                    let val = serde_json::to_value(&profile).map_err(|e| {
                        rcm_core::CoreError::Serialization(e.to_string())
                    })?;
                    ipc.call("profiles.save_mount", val).await?;
                    let _ = ipc.call("reconcile.now", serde_json::json!({})).await;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Save mount failed: {}", e)))?
    }

    pub async fn delete_mount_profile(&self, id: &rcm_core::Id) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        let id_str = id.to_string();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref ipc) = s.ipc_client {
                    ipc.call("profiles.delete_mount", serde_json::Value::String(id_str)).await?;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Delete mount failed: {}", e)))?
    }

    pub async fn mount_drive(
        &self,
        remote: &str,
        mount_point: &str,
        preset: rcm_core::MountPreset,
    ) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        let r = remote.to_string();
        let mp = mount_point.to_string();
        let opts = preset.default_options();

        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref rc) = s.rc_client {
                    rc.mount_mount(&r, &mp, Some("cmount"), opts)
                        .await
                        .map_err(|e| rcm_core::CoreError::Validation(e.to_string()))?;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Mount failed: {}", e)))?
    }

    pub async fn unmount_drive(&self, mount_point: &str) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        let mp = mount_point.to_string();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref rc) = s.rc_client {
                    rc.mount_unmount(&mp)
                        .await
                        .map_err(|e| rcm_core::CoreError::Validation(e.to_string()))?;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Unmount failed: {}", e)))?
    }

    pub async fn delete_remote(&self, name: &str) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        let n = name.to_string();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref rc) = s.rc_client {
                    rc.config_delete(&n)
                        .await
                        .map_err(|e| rcm_core::CoreError::Validation(e.to_string()))?;
                    let _ = rc.fscache_clear().await;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Delete remote failed: {}", e)))?
    }

    pub async fn fetch_providers(&self) -> Result<Vec<rcm_rc::types::ProviderInfo>, rcm_core::CoreError> {
        let app = self.clone();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref rc) = s.rc_client {
                    let providers = rc.config_providers().await.map_err(|e| {
                        rcm_core::CoreError::Validation(e.to_string())
                    })?;
                    return Ok(providers);
                }
                Ok(Vec::new())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Fetch providers failed: {}", e)))?
    }

    pub async fn list_files(&self, fs: &str, remote: &str) -> Result<Vec<FileEntryViewModel>, rcm_core::CoreError> {
        let app = self.clone();
        let fs_str = fs.to_string();
        let rem_str = remote.to_string();

        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref rc) = s.rc_client {
                    let resp = rc.operations_list(&fs_str, &rem_str).await.map_err(|e| {
                        rcm_core::CoreError::Validation(e.to_string())
                    })?;
                    let mut entries = Vec::new();
                    for item in resp.list {
                        entries.push(FileEntryViewModel {
                            name: item.name,
                            path: item.path,
                            size_bytes: item.size,
                            is_dir: item.is_dir,
                            mod_time: item.mod_time,
                            mime_type: item.mime_type,
                        });
                    }
                    return Ok(entries);
                }
                Ok(Vec::new())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("List files failed: {}", e)))?
    }

    pub async fn start_serve(
        &self,
        profile: rcm_core::ServeProfile,
    ) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref ipc) = s.ipc_client {
                    let val = serde_json::to_value(&profile).map_err(|e| {
                        rcm_core::CoreError::Serialization(e.to_string())
                    })?;
                    ipc.call("profiles.save_serve", val).await?;
                }
                if let Some(ref rc) = s.rc_client {
                    rc.serve_start(
                        &profile.protocol.to_string(),
                        &profile.remote,
                        &profile.addr,
                        profile.user.as_deref(),
                        profile.pass.as_deref(),
                        profile.vfs_options.clone(),
                    )
                    .await
                    .map_err(|e| rcm_core::CoreError::Validation(e.to_string()))?;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Start serve failed: {}", e)))?
    }

    pub async fn stop_serve(&self, id: u64) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref rc) = s.rc_client {
                    rc.serve_stop(id)
                        .await
                        .map_err(|e| rcm_core::CoreError::Validation(e.to_string()))?;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Stop serve failed: {}", e)))?
    }

    pub async fn fetch_snapshots(&self) -> Result<Vec<rcm_core::SnapshotMeta>, rcm_core::CoreError> {
        let app = self.clone();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref ipc) = s.ipc_client {
                    let res = ipc.call("backups.list", serde_json::json!({})).await?;
                    let list: Vec<rcm_core::SnapshotMeta> = serde_json::from_value(res).unwrap_or_default();
                    return Ok(list);
                }
                Ok(Vec::new())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Fetch snapshots failed: {}", e)))?
    }

    pub async fn restore_snapshot(&self, filename: &str) -> Result<(), rcm_core::CoreError> {
        let app = self.clone();
        let fname = filename.to_string();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref ipc) = s.ipc_client {
                    ipc.call("backups.restore", serde_json::Value::String(fname)).await?;
                }
                Ok::<(), rcm_core::CoreError>(())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Restore snapshot failed: {}", e)))?
    }

    pub async fn update_rclone(&self) -> Result<String, rcm_core::CoreError> {
        let app = self.clone();
        self.tokio_handle
            .spawn(async move {
                let s = app.state.read().await;
                if let Some(ref ipc) = s.ipc_client {
                    let res = ipc.call("binary.fetch", serde_json::Value::Null).await?;
                    let ver = res.get("version").and_then(|v| v.as_str()).unwrap_or("latest").to_string();
                    return Ok(ver);
                }
                Ok("unknown".to_string())
            })
            .await
            .map_err(|e| rcm_core::CoreError::Connection(format!("Update rclone failed: {}", e)))?
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

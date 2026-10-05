use std::sync::Arc;
use std::time::Instant;
use camino::Utf8PathBuf;
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{info, warn};

use rcm_core::{CoreError, DaemonState, MountProfile, RcmEvent, ServeProfile};
use rcm_rc::RcClient;
use crate::binary::BinaryManager;
use crate::health::HealthMonitor;
use crate::logs::LogRingBuffer;
use crate::process::RcdProcessManager;
use crate::reconciler::{Reconciler, ReconcilerAction};

pub struct Supervisor {
    binary_mgr: Arc<Mutex<BinaryManager>>,
    process_mgr: Arc<RcdProcessManager>,
    health_mon: Arc<RwLock<HealthMonitor>>,
    log_buffer: Arc<LogRingBuffer>,
    rc_client: Arc<RwLock<Option<RcClient>>>,
    config_path: Utf8PathBuf,
    event_tx: broadcast::Sender<RcmEvent>,
}

impl Supervisor {
    pub fn new(
        state_dir: Utf8PathBuf,
        config_path: Utf8PathBuf,
        binary_mgr: BinaryManager,
        event_tx: broadcast::Sender<RcmEvent>,
    ) -> Self {
        let log_buffer = Arc::new(LogRingBuffer::default());
        let process_mgr = Arc::new(RcdProcessManager::new(&state_dir, log_buffer.clone()));

        Self {
            binary_mgr: Arc::new(Mutex::new(binary_mgr)),
            process_mgr,
            health_mon: Arc::new(RwLock::new(HealthMonitor::new())),
            log_buffer,
            rc_client: Arc::new(RwLock::new(None)),
            config_path,
            event_tx,
        }
    }

    pub fn log_buffer(&self) -> Arc<LogRingBuffer> {
        self.log_buffer.clone()
    }

    pub async fn fetch_rclone(&self, version: Option<&str>) -> Result<String, CoreError> {
        let mut mgr = self.binary_mgr.lock().await;
        mgr.fetch_and_install_rclone(version)
    }

    pub async fn register_rclone(&self, path: &camino::Utf8Path) -> Result<String, CoreError> {
        let mut mgr = self.binary_mgr.lock().await;
        mgr.register_existing_binary(path)
    }

    pub async fn current_binary_version(&self) -> Option<String> {
        let mgr = self.binary_mgr.lock().await;
        mgr.current_version().map(String::from)
    }

    pub async fn current_state(&self) -> DaemonState {
        self.health_mon.read().await.current_state().clone()
    }

    pub async fn get_client(&self) -> Option<RcClient> {
        self.rc_client.read().await.clone()
    }

    /// Starts daemon: tries adoption first (DM-5), otherwise spawns new (DM-1, DM-2)
    pub async fn start(&self) -> Result<(), CoreError> {
        let bin_path = {
            let mut mgr = self.binary_mgr.lock().await;
            if mgr.get_current_binary_path().is_none() {
                let _ = mgr.scan_and_auto_adopt();
            }
            if mgr.get_current_binary_path().is_none() {
                info!("No rclone binary registered. Automatically fetching latest stable rclone...");
                let _ = mgr.fetch_and_install_rclone(None);
            }
            mgr.get_current_binary_path().ok_or_else(|| {
                CoreError::NotFound("No registered rclone binary found".to_string())
            })?
        };

        self.set_state(DaemonState::Starting).await;

        // Try adopt first (DM-5)
        if let Some(client) = self.process_mgr.try_adopt(&bin_path, &self.config_path).await {
            let pid = client.pid().await.unwrap_or(0);
            let ver = client.version().await.map(|v| v.version).unwrap_or_default();
            let base_url = client.base_url().to_string();

            let ready_state = DaemonState::Ready {
                execute_id: ver.clone(),
                version: ver,
                pid,
                addr: base_url,
            };

            *self.rc_client.write().await = Some(client);
            self.set_state(ready_state).await;
            return Ok(());
        }

        // Spawn new rcd
        match self.process_mgr.spawn(&bin_path, &self.config_path, None).await {
            Ok(client) => {
                let pid = client.pid().await.unwrap_or(0);
                let ver = client.version().await.map(|v| v.version).unwrap_or_default();
                let base_url = client.base_url().to_string();

                let ready_state = DaemonState::Ready {
                    execute_id: ver.clone(),
                    version: ver,
                    pid,
                    addr: base_url,
                };

                *self.rc_client.write().await = Some(client);
                self.set_state(ready_state).await;
                Ok(())
            }
            Err(e) => {
                let crash_state = self
                    .health_mon
                    .write()
                    .await
                    .on_crash(Instant::now(), None, e.to_string());
                self.set_state(crash_state).await;
                Err(e)
            }
        }
    }

    /// Stops daemon gracefully (DM-1)
    pub async fn stop(&self) -> Result<(), CoreError> {
        self.set_state(DaemonState::Stopping).await;
        let client_opt = self.rc_client.read().await.clone();
        self.process_mgr.stop(client_opt.as_ref()).await?;
        *self.rc_client.write().await = None;
        self.set_state(DaemonState::Stopped).await;
        Ok(())
    }

    /// Restarts daemon (DM-1)
    pub async fn restart(&self) -> Result<(), CoreError> {
        self.stop().await?;
        self.start().await
    }

    /// Runs one pass of desired-state reconciliation (§7.6)
    pub async fn reconcile(
        &self,
        desired_mounts: &[MountProfile],
        desired_serves: &[ServeProfile],
    ) -> Result<Vec<ReconcilerAction>, CoreError> {
        let client = match self.get_client().await {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };

        let actual_mounts = client.mount_list_mounts().await.unwrap_or_default();
        let actual_serves = client.serve_list().await.unwrap_or_default();

        let actions = Reconciler::compute_actions(
            desired_mounts,
            desired_serves,
            &actual_mounts,
            &actual_serves,
        );

        for action in &actions {
            match action {
                ReconcilerAction::StartMount(m) => {
                    info!("Reconciler starting mount '{}' ({})", m.name, m.remote);
                    let mount_point = match &m.target {
                        rcm_core::MountTarget::DriveLetter(c) => format!("{}:", c),
                        rcm_core::MountTarget::AutoDriveLetter => "*".to_string(),
                        rcm_core::MountTarget::Folder(p) => p.as_str().to_string(),
                        rcm_core::MountTarget::Unc(u) => u.clone(),
                    };
                    let _ = client
                        .mount_mount(&m.remote, &mount_point, Some(&m.mount_type), m.options.clone())
                        .await;
                }
                ReconcilerAction::StartServe(s) => {
                    info!("Reconciler starting serve '{}' ({})", s.name, s.remote);
                    let _ = client
                        .serve_start(
                            &s.protocol.to_string(),
                            &s.remote,
                            &s.addr,
                            s.user.as_deref(),
                            s.pass.as_deref(),
                            s.vfs_options.clone(),
                        )
                        .await;
                }
                ReconcilerAction::MarkUnmanagedMount(info) => {
                    warn!("Detected unmanaged mount at {}", info.mount_point);
                }
                ReconcilerAction::MarkUnmanagedServe(info) => {
                    warn!("Detected unmanaged serve on {}", info.addr);
                }
            }
        }

        Ok(actions)
    }

    async fn set_state(&self, state: DaemonState) {
        self.health_mon.write().await.set_state(state.clone());
        let _ = self.event_tx.send(RcmEvent::DaemonStateChanged(state));
    }
}

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use camino::{Utf8Path, Utf8PathBuf};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};

use rcm_core::{CoreError, DaemonAdoptInfo};
use rcm_rc::RcClient;
use crate::logs::LogRingBuffer;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct RcdProcessManager {
    child: Arc<Mutex<Option<Child>>>,
    adopt_info: Arc<Mutex<Option<DaemonAdoptInfo>>>,
    log_buffer: Arc<LogRingBuffer>,
    state_file: Utf8PathBuf,
}

impl RcdProcessManager {
    pub fn new(state_dir: &Utf8Path, log_buffer: Arc<LogRingBuffer>) -> Self {
        let state_file = state_dir.join("rcd.json");
        Self {
            child: Arc::new(Mutex::new(None)),
            adopt_info: Arc::new(Mutex::new(None)),
            log_buffer,
            state_file,
        }
    }

    /// Finds a free TCP port on localhost for rcd loopback transport (R7, §7.1)
    pub fn find_free_port() -> Result<u16, CoreError> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| {
            CoreError::Validation(format!("Failed to find free port for rcd: {}", e))
        })?;
        let port = listener.local_addr().map_err(|e| {
            CoreError::Validation(format!("Failed to get local port: {}", e))
        })?.port();
        drop(listener);
        Ok(port)
    }

    /// Spawns rcd process with per-session credentials and explicit transport (DM-2, §7.1)
    pub async fn spawn(
        &self,
        rclone_exe: &Utf8Path,
        config_path: &Utf8Path,
        password_cmd: Option<&str>,
    ) -> Result<RcClient, CoreError> {
        let port = Self::find_free_port()?;
        let addr = format!("127.0.0.1:{}", port);
        let base_url = format!("http://{}", addr);

        // Generate per-session random credentials (threat model §9)
        let auth_user = format!("rcm_{}", uuid::Uuid::new_v4().simple());
        let auth_pass = format!("rcm_pass_{}", uuid::Uuid::new_v4());

        let mut cmd = Command::new(rclone_exe.as_std_path());
        cmd.arg("rcd")
            .arg("--config")
            .arg(config_path.as_std_path())
            .arg("--rc-addr")
            .arg(&addr)
            .arg("--rc-job-expire-duration")
            .arg("1h")
            .arg("--use-json-log")
            .arg("--log-level")
            .arg("NOTICE");

        if let Some(cmd_str) = password_cmd {
            cmd.arg("--password-command").arg(cmd_str);
        }

        // Credentials passed through environment variables, NOT argv (threat model §9)
        cmd.env("RCLONE_RC_USER", &auth_user);
        cmd.env("RCLONE_RC_PASS", &auth_pass);

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            CoreError::Validation(format!("Failed to spawn rclone rcd from '{}': {}", rclone_exe, e))
        })?;

        // Stream stdout and stderr into ring buffer (DM-9)
        if let Some(stdout) = child.stdout.take() {
            let buf = self.log_buffer.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    buf.push(line);
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let buf = self.log_buffer.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    buf.push(line);
                }
            });
        }

        let pid = child.id();
        info!("Spawned rclone rcd (PID {}) on {}", pid, addr);

        let now_utc = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let client = RcClient::new(base_url.clone(), Some(&auth_user), Some(&auth_pass));

        // Wait for rcd to become ready via rc/noop
        let mut ready = false;
        for _ in 0..50 {
            sleep(Duration::from_millis(100)).await;
            if client.noop().await.is_ok() {
                ready = true;
                break;
            }
        }

        if !ready {
            let _ = child.kill();
            return Err(CoreError::Validation(
                "Timed out waiting for rclone rcd to respond to rc/noop".to_string(),
            ));
        }

        let execute_id = client.version().await.map(|v| v.version).unwrap_or_default();

        let adopt_info = DaemonAdoptInfo {
            pid,
            start_time: now_utc,
            exe_path: rclone_exe.to_path_buf(),
            config_path: config_path.to_path_buf(),
            addr: base_url,
            execute_id,
            auth_user,
            auth_pass,
        };

        // Save rcd.json state for adoption (DM-5)
        self.save_adopt_info(&adopt_info).await?;

        *self.child.lock().await = Some(child);
        *self.adopt_info.lock().await = Some(adopt_info);

        Ok(client)
    }

    /// Attempts to adopt an already-running compatible rcd process (DM-5)
    pub async fn try_adopt(
        &self,
        expected_exe: &Utf8Path,
        expected_config: &Utf8Path,
    ) -> Option<RcClient> {
        if !self.state_file.exists() {
            return None;
        }

        let data = tokio::fs::read(&self.state_file).await.ok()?;
        let info: DaemonAdoptInfo = serde_json::from_slice(&data).ok()?;

        // Verify binary path and config path match
        if info.exe_path != expected_exe || info.config_path != expected_config {
            return None;
        }

        let client = RcClient::new(&info.addr, Some(&info.auth_user), Some(&info.auth_pass));

        // Probe with rc/noop
        if client.noop().await.is_ok() {
            info!("Successfully adopted running rclone rcd (PID {})", info.pid);
            *self.adopt_info.lock().await = Some(info);
            return Some(client);
        }

        // Stale state file; remove it
        let _ = tokio::fs::remove_file(&self.state_file).await;
        None
    }

    /// Graceful stop: unmountall -> quit -> timeout wait -> terminate (DM-1)
    pub async fn stop(&self, client: Option<&RcClient>) -> Result<(), CoreError> {
        if let Some(c) = client {
            let _ = c.mount_unmount_all().await;
            let _ = c.quit().await;
        }

        // Wait up to 10 seconds for child to exit
        let start = Instant::now();
        loop {
            let mut child_guard = self.child.lock().await;
            if let Some(ref mut c) = *child_guard {
                match c.try_wait() {
                    Ok(Some(_)) => {
                        *child_guard = None;
                        break;
                    }
                    Ok(None) => {
                        if start.elapsed() > Duration::from_secs(10) {
                            warn!("rcd did not exit gracefully within 10s, killing PID {}", c.id());
                            let _ = c.kill();
                            let _ = c.wait();
                            *child_guard = None;
                            break;
                        }
                    }
                    Err(_) => {
                        *child_guard = None;
                        break;
                    }
                }
            } else {
                break;
            }
            drop(child_guard);
            sleep(Duration::from_millis(150)).await;
        }

        *self.adopt_info.lock().await = None;
        let _ = tokio::fs::remove_file(&self.state_file).await;
        Ok(())
    }

    async fn save_adopt_info(&self, info: &DaemonAdoptInfo) -> Result<(), CoreError> {
        let data = serde_json::to_vec_pretty(info).map_err(|e| {
            CoreError::Serialization(format!("Failed to serialize adopt info: {}", e))
        })?;
        if let Some(parent) = self.state_file.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        tokio::fs::write(&self.state_file, data).await.map_err(|e| {
            CoreError::Validation(format!("Failed to write rcd.json: {}", e))
        })
    }
}

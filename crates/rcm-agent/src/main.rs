use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use camino::Utf8PathBuf;
use clap::Parser;
use tokio::sync::{broadcast, RwLock};
use tokio::time::sleep;
use tracing::{info, warn};

use rcm_core::{CoreError, Id, MountProfile, RcmEvent, ServeProfile};
use rcm_config::{ConfigWatcher, ProfileStore, RestoreManager, SnapshotStore};
use rcm_ipc::protocol::{IpcError, IpcRequest, RcConnectionInfo};
use rcm_ipc::IpcServer;
use rcm_platform::paths::RcmPaths;
use rcm_supervisor::binary::BinaryManager;
use rcm_supervisor::Supervisor;

#[derive(Parser, Debug)]
#[command(name = "rcm-agent", about = "Rclone Manager background agent")]
struct CliArgs {
    #[arg(long, help = "Run in background")]
    background: bool,

    #[arg(long, help = "Path to rclone.conf")]
    config: Option<Utf8PathBuf>,

    #[arg(long, help = "Custom IPC pipe/socket name")]
    pipe: Option<String>,

    #[arg(long, help = "Multi-call invocation as rcmctl")]
    ctl: bool,

    #[arg(trailing_var_arg = true)]
    ctl_args: Vec<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    // Check multi-call mode (SRDD §7.15)
    let exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_default();

    if args.ctl || exe_name.starts_with("rcmctl") {
        // Multi-call delegated to CLI handler
        return run_ctl(args.ctl_args).await;
    }

    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Rclone Manager Agent (v{})", env!("CARGO_PKG_VERSION"));

    let paths = RcmPaths::resolve()?;
    paths.ensure_dirs()?;

    let config_path = args
        .config
        .unwrap_or_else(|| paths.default_rclone_config_path().clone());

    let (event_tx, _) = broadcast::channel(128);

    let bin_mgr = BinaryManager::new(paths.data_dir());

    let supervisor = Arc::new(Supervisor::new(
        paths.state_dir().clone(),
        config_path.clone(),
        bin_mgr,
        event_tx.clone(),
    ));

    let snapshot_store = Arc::new(SnapshotStore::new(paths.backups_dir()));
    let is_restoring = Arc::new(AtomicBool::new(false));

    let profile_store = Arc::new(RwLock::new(ProfileStore::load_or_create(
        &paths.profiles_file_path(),
    )?));

    // File watcher on rclone.conf (CF-9)
    let mut watcher = ConfigWatcher::new(config_path.clone(), snapshot_store.clone());
    let w_ev_tx = event_tx.clone();
    let w_conf_path = config_path.clone();
    let _ = watcher.start_watch(move || {
        let _ = w_ev_tx.send(RcmEvent::ExternalConfigDetected {
            path: w_conf_path.clone(),
        });
    });

    // Start background health polling (DM-3: 5s health check)
    let h_sup = supervisor.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(5)).await;
            if let Some(client) = h_sup.get_client().await {
                if let Err(e) = client.noop().await {
                    warn!("Periodic health check rc/noop failed: {}", e);
                }
            }
        }
    });

    // Determine pipe name
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    let pipe_name = args
        .pipe
        .unwrap_or_else(|| format!("rcm-{}", username));

    let ipc_server = IpcServer::bind(&pipe_name).await?;

    info!("Agent ready. Listening on IPC socket '{}'", pipe_name);

    // Run IPC dispatch
    let s_sup = supervisor.clone();
    let s_snap = snapshot_store.clone();
    let s_prof = profile_store.clone();
    let s_restore = is_restoring.clone();
    let s_conf_path = config_path.clone();

    ipc_server
        .run(move |req: IpcRequest| {
            let sup = s_sup.clone();
            let snap = s_snap.clone();
            let prof = s_prof.clone();
            let s_restore = s_restore.clone();
            let s_conf_path = s_conf_path.clone();

            async move {
                match req.method.as_str() {
                    "daemon.status" => {
                        let state = sup.current_state().await;
                        Ok(serde_json::to_value(state).unwrap())
                    }
                    "daemon.start" => {
                        sup.start().await.map_err(to_ipc_error)?;
                        Ok(serde_json::Value::Bool(true))
                    }
                    "daemon.stop" => {
                        sup.stop().await.map_err(to_ipc_error)?;
                        Ok(serde_json::Value::Bool(true))
                    }
                    "daemon.restart" => {
                        sup.restart().await.map_err(to_ipc_error)?;
                        Ok(serde_json::Value::Bool(true))
                    }
                    "rc.connection_info" => {
                        if let Some(client) = sup.get_client().await {
                            let info = RcConnectionInfo {
                                addr: client.base_url().to_string(),
                                auth_user: None,
                                auth_pass: None,
                            };
                            Ok(serde_json::to_value(info).unwrap())
                        } else {
                            Err(IpcError {
                                code: -32001,
                                message: "rcd is not running".to_string(),
                            })
                        }
                    }
                    "profiles.list_mounts" => {
                        let p = prof.read().await;
                        Ok(serde_json::to_value(p.list_mounts()).unwrap())
                    }
                    "profiles.save_mount" => {
                        let mount: MountProfile = serde_json::from_value(req.params).map_err(|e| {
                            IpcError { code: -32602, message: e.to_string() }
                        })?;
                        prof.write().await.save_mount(mount).map_err(to_ipc_error)?;
                        Ok(serde_json::Value::Bool(true))
                    }
                    "profiles.delete_mount" => {
                        let id_str: String = serde_json::from_value(req.params).map_err(|e| {
                            IpcError { code: -32602, message: e.to_string() }
                        })?;
                        let id = Id::from_string(id_str);
                        prof.write().await.delete_mount(&id).map_err(to_ipc_error)?;
                        Ok(serde_json::Value::Bool(true))
                    }
                    "profiles.list_serves" => {
                        let p = prof.read().await;
                        Ok(serde_json::to_value(p.list_serves()).unwrap())
                    }
                    "profiles.save_serve" => {
                        let serve: ServeProfile = serde_json::from_value(req.params).map_err(|e| {
                            IpcError { code: -32602, message: e.to_string() }
                        })?;
                        prof.write().await.save_serve(serve).map_err(to_ipc_error)?;
                        Ok(serde_json::Value::Bool(true))
                    }
                    "profiles.delete_serve" => {
                        let id_str: String = serde_json::from_value(req.params).map_err(|e| {
                            IpcError { code: -32602, message: e.to_string() }
                        })?;
                        let id = Id::from_string(id_str);
                        prof.write().await.delete_serve(&id).map_err(to_ipc_error)?;
                        Ok(serde_json::Value::Bool(true))
                    }
                    "reconcile.now" => {
                        let (mounts, serves) = {
                            let p = prof.read().await;
                            (p.list_mounts().to_vec(), p.list_serves().to_vec())
                        };
                        let actions = sup.reconcile(&mounts, &serves).await.map_err(to_ipc_error)?;
                        Ok(serde_json::json!({ "reconciled_actions": actions.len() }))
                    }
                    "backups.list" => {
                        let list = snap.list_snapshots().await.map_err(to_ipc_error)?;
                        Ok(serde_json::to_value(list).unwrap())
                    }
                    "backups.restore" => {
                        let filename: String = serde_json::from_value(req.params).map_err(|e| {
                            IpcError { code: -32602, message: e.to_string() }
                        })?;
                        s_restore.store(true, std::sync::atomic::Ordering::SeqCst);
                        let _ = sup.stop().await;
                        let mgr = RestoreManager::new(SnapshotStore::new(s_conf_path.parent().unwrap_or(&s_conf_path)));
                        let res = mgr.swap_restore(&filename, &s_conf_path).await.map_err(to_ipc_error);
                        let _ = sup.start().await;
                        s_restore.store(false, std::sync::atomic::Ordering::SeqCst);
                        res?;
                        Ok(serde_json::Value::Bool(true))
                    }
                    "logs.recent" => {
                        let logs = sup.log_buffer().get_recent();
                        Ok(serde_json::to_value(logs).unwrap())
                    }
                    "binary.fetch" => {
                        let ver_param: Option<String> = serde_json::from_value(req.params).ok().flatten();
                        let installed_ver = sup.fetch_rclone(ver_param.as_deref()).await.map_err(to_ipc_error)?;
                        Ok(serde_json::json!({ "version": installed_ver }))
                    }
                    "binary.register" => {
                        let path_str: String = serde_json::from_value(req.params).map_err(|e| {
                            IpcError { code: -32602, message: e.to_string() }
                        })?;
                        let p = Utf8PathBuf::from(path_str);
                        let ver = sup.register_rclone(&p).await.map_err(to_ipc_error)?;
                        Ok(serde_json::json!({ "version": ver }))
                    }
                    "binary.status" => {
                        let cur = sup.current_binary_version().await;
                        Ok(serde_json::json!({ "current": cur }))
                    }
                    _ => Err(IpcError {
                        code: -32601,
                        message: format!("Unknown method: {}", req.method),
                    }),
                }
            }
        })
        .await;

    Ok(())
}

fn to_ipc_error(e: CoreError) -> IpcError {
    IpcError {
        code: -32000,
        message: e.to_string(),
    }
}

async fn run_ctl(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    println!("rcmctl running with args: {:?}", args);
    Ok(())
}

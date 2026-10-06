use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use rcm_core::DaemonState;
use rcm_ipc::protocol::{IpcError, IpcRequest, RcConnectionInfo};
use rcm_ipc::IpcServer;
use rcm_ui::app::AppController;

/// Traceability: FR-LC-01, FR-LC-03, NFR-TH-01, NFR-TH-03
/// Tests zero-friction auto-connect, auto-spawn, and auto-bootstrap of stopped daemon
#[tokio::test]
async fn test_fr_lc_01_lc_03_nfr_th_01_auto_bootstrap_daemon_if_stopped() {
    let pipe_name = format!("rcm-test-bootstrap-{}", uuid::Uuid::new_v4());
    let server = IpcServer::bind(&pipe_name).await.expect("Bind failed");

    let is_started = Arc::new(AtomicBool::new(false));
    let has_fetched = Arc::new(AtomicBool::new(false));

    let s_started = is_started.clone();
    let s_fetched = has_fetched.clone();

    let srv_handle = tokio::spawn(async move {
        server.run(move |req: IpcRequest| {
            let started = s_started.clone();
            let fetched = s_fetched.clone();
            async move {
                match req.method.as_str() {
                    "daemon.status" => {
                        if started.load(Ordering::SeqCst) {
                            let state = DaemonState::Ready {
                                execute_id: "exec-test".to_string(),
                                version: "v1.75.1".to_string(),
                                pid: 1111,
                                addr: "http://127.0.0.1:5572".to_string(),
                            };
                            Ok(serde_json::to_value(state).unwrap())
                        } else {
                            Ok(serde_json::to_value(DaemonState::Stopped).unwrap())
                        }
                    }
                    "daemon.start" => {
                        if !fetched.load(Ordering::SeqCst) {
                            Err(IpcError {
                                code: -32000,
                                message: "Resource not found: No registered rclone binary found".to_string(),
                            })
                        } else {
                            started.store(true, Ordering::SeqCst);
                            Ok(serde_json::Value::Bool(true))
                        }
                    }
                    "binary.fetch" => {
                        fetched.store(true, Ordering::SeqCst);
                        Ok(serde_json::json!({ "version": "v1.75.1" }))
                    }
                    "rc.connection_info" => {
                        let info = RcConnectionInfo {
                            addr: "http://127.0.0.1:5572".to_string(),
                            auth_user: Some("test".to_string()),
                            auth_pass: Some("test".to_string()),
                        };
                        Ok(serde_json::to_value(info).unwrap())
                    }
                    _ => Err(IpcError { code: -32601, message: "Not found".to_string() }),
                }
            }
        }).await;
    });

    sleep(Duration::from_millis(50)).await;

    let app = AppController::new();
    let bootstrap_res = app.ensure_connected_and_ready(&pipe_name).await;
    assert!(bootstrap_res.is_ok(), "Bootstrap should succeed: {:?}", bootstrap_res);

    let state = app.state().read().await.daemon_state.clone();
    assert!(state.is_ready());

    srv_handle.abort();
}

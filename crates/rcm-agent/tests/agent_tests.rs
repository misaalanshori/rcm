use std::sync::Arc;
use std::time::Duration;
use camino::Utf8PathBuf;
use tokio::sync::broadcast;
use tokio::time::sleep;

use rcm_core::DaemonState;
use rcm_supervisor::binary::BinaryManager;
use rcm_supervisor::Supervisor;
use rcm_rc::MockRcServer;
use rcm_ipc::client::IpcClient;
use rcm_ipc::protocol::RcConnectionInfo;

/// Traceability: FR-LC-05
/// Verifies background agent IPC service endpoints for status and RC connection info
#[tokio::test]
async fn test_fr_lc_05_agent_ipc_service_endpoints() {
    let mock = MockRcServer::start().await;
    mock.set_route("core/version", serde_json::json!({
        "version": "v1.75.1",
        "decomposed": [1, 75, 1],
        "isBeta": false,
        "os": "windows",
        "arch": "amd64"
    })).await;
    mock.set_route("core/pid", serde_json::json!({ "pid": 4321 })).await;
    mock.set_route("mount/listmounts", serde_json::json!({ "mountPoints": [] })).await;
    mock.set_route("serve/list", serde_json::json!({ "serves": [] })).await;

    let temp_dir = Utf8PathBuf::from_path_buf(
        std::env::temp_dir().join(format!("rcm_test_agent_{}", uuid::Uuid::new_v4())),
    ).unwrap();
    let _ = std::fs::create_dir_all(&temp_dir);

    let state_dir = temp_dir.join("state");
    let config_path = temp_dir.join("rclone.conf");
    std::fs::write(&config_path, "").unwrap();

    let bin_mgr = BinaryManager::new(&temp_dir);
    let (event_tx, _) = broadcast::channel(32);

    let supervisor = Arc::new(Supervisor::new(
        state_dir,
        config_path,
        bin_mgr,
        event_tx.clone(),
    ));

    let pipe_name = format!("rcm-test-agent-pipe-{}", uuid::Uuid::new_v4());
    let server = rcm_ipc::IpcServer::bind(&pipe_name).await.expect("Bind failed");

    let s_sup = supervisor.clone();
    let mock_url = mock.base_url();

    let srv_handle = tokio::spawn(async move {
        server.run(move |req| {
            let sup = s_sup.clone();
            let url = mock_url.clone();
            async move {
                match req.method.as_str() {
                    "daemon.status" => {
                        let state = sup.current_state().await;
                        Ok(serde_json::to_value(state).unwrap())
                    }
                    "rc.connection_info" => {
                        let info = RcConnectionInfo {
                            addr: url,
                            auth_user: Some("test_user".to_string()),
                            auth_pass: Some("test_pass".to_string()),
                        };
                        Ok(serde_json::to_value(info).unwrap())
                    }
                    _ => Err(rcm_ipc::protocol::IpcError {
                        code: -32601,
                        message: "Not found".to_string(),
                    }),
                }
            }
        }).await;
    });

    sleep(Duration::from_millis(50)).await;

    let client = IpcClient::connect(&pipe_name).await.expect("Client connect failed");

    // Check status via IPC
    let status_val = client.call("daemon.status", serde_json::json!({})).await.unwrap();
    let state: DaemonState = serde_json::from_value(status_val).unwrap();
    assert_eq!(state, DaemonState::Stopped);

    // Check rc connection info
    let conn_val = client.call("rc.connection_info", serde_json::json!({})).await.unwrap();
    let conn_info: RcConnectionInfo = serde_json::from_value(conn_val).unwrap();
    assert!(!conn_info.addr.is_empty());

    srv_handle.abort();
    mock.stop();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

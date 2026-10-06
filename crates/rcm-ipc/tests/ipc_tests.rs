use std::time::Duration;
use tokio::time::sleep;
use rcm_core::DaemonState;
use rcm_ipc::protocol::RcConnectionInfo;
use rcm_ipc::server::IpcServer;
use rcm_ipc::client::IpcClient;

/// Traceability: FR-LC-02, NFR-SC-02
/// Tests local IPC protocol handshake, request/response, and error handling over named pipes
#[tokio::test]
async fn test_fr_lc_02_nfr_sc_02_ipc_handshake_and_request_response() {
    let pipe_name = format!("rcm-test-pipe-{}", uuid::Uuid::new_v4());

    let server = IpcServer::bind(&pipe_name).await.expect("Failed to bind IPC server");

    let srv_handle = tokio::spawn(async move {
        server.run(|req| async move {
            match req.method.as_str() {
                "daemon.status" => {
                    let state = DaemonState::Ready {
                        execute_id: "exec-1".to_string(),
                        version: "v1.75.1".to_string(),
                        pid: 9999,
                        addr: "http://127.0.0.1:5572".to_string(),
                    };
                    Ok(serde_json::to_value(state).unwrap())
                }
                "rc.connection_info" => {
                    let info = RcConnectionInfo {
                        addr: "http://127.0.0.1:5572".to_string(),
                        auth_user: Some("rcm_user".to_string()),
                        auth_pass: Some("rcm_pass".to_string()),
                    };
                    Ok(serde_json::to_value(info).unwrap())
                }
                _ => Err(rcm_ipc::protocol::IpcError {
                    code: -32601,
                    message: format!("Method not found: {}", req.method),
                }),
            }
        }).await;
    });

    sleep(Duration::from_millis(50)).await;

    let client = IpcClient::connect(&pipe_name).await.expect("Failed to connect IPC client");

    // 1. Test daemon.status
    let status_val = client.call("daemon.status", serde_json::json!({})).await.expect("daemon.status failed");
    let state: DaemonState = serde_json::from_value(status_val).expect("Invalid state");
    assert!(state.is_ready());

    // 2. Test rc.connection_info
    let conn_val = client.call("rc.connection_info", serde_json::json!({})).await.expect("connection_info failed");
    let conn_info: RcConnectionInfo = serde_json::from_value(conn_val).expect("Invalid conn info");
    assert_eq!(conn_info.addr, "http://127.0.0.1:5572");
    assert_eq!(conn_info.auth_user, Some("rcm_user".to_string()));

    // 3. Unknown method
    let unknown_res = client.call("nonexistent.method", serde_json::json!({})).await;
    assert!(unknown_res.is_err());

    srv_handle.abort();
}

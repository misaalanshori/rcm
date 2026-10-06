use std::collections::BTreeMap;
use rcm_rc::{MockRcServer, RcClient, WizardStep};

/// Traceability: FR-LC-06, FR-FL-02
/// Verifies core RC client endpoints (version, pid, obscure, listremotes, listmounts)
#[tokio::test]
async fn test_fr_lc_06_rc_client_core_calls() {
    let mock = MockRcServer::start().await;

    mock.set_route("core/version", serde_json::json!({
        "version": "v1.75.1",
        "decomposed": [1, 75, 1],
        "isBeta": false,
        "os": "windows",
        "arch": "amd64",
        "goVersion": "go1.23.0"
    })).await;

    mock.set_route("core/pid", serde_json::json!({ "pid": 12345 })).await;
    mock.set_route("core/obscure", serde_json::json!({ "obscured": "xyzObscuredPass" })).await;
    mock.set_route("config/listremotes", serde_json::json!({ "remotes": ["remote1", "remote2"] })).await;
    mock.set_route("mount/listmounts", serde_json::json!({
        "mountPoints": [
            { "Fs": "gdrive:", "MountPoint": "G:", "MountedOn": "2026-10-05T12:00:00Z", "Type": "cmount" }
        ]
    })).await;

    let client = RcClient::new(mock.base_url(), Some("rcm_user"), Some("rcm_pass"));

    // 1. Version
    let ver = client.version().await.unwrap();
    assert_eq!(ver.version, "v1.75.1");
    assert_eq!(ver.os, "windows");

    // 2. Pid
    let pid = client.pid().await.unwrap();
    assert_eq!(pid, 12345);

    // 3. Obscure
    let obscured = client.obscure("mySecret").await.unwrap();
    assert_eq!(obscured, "xyzObscuredPass");

    // 4. List remotes
    let remotes = client.config_list_remotes().await.unwrap();
    assert_eq!(remotes, vec!["remote1", "remote2"]);

    // 5. Mount list
    let mounts = client.mount_list_mounts().await.unwrap();
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0].mount_point, "G:");
    assert_eq!(mounts[0].fs, "gdrive:");

    mock.stop();
}

/// Traceability: FR-RM-05, FR-RM-08
/// Verifies wizard state machine step sequence, question extraction, and answer passing
#[tokio::test]
async fn test_fr_rm_05_rm_08_wizard_driver_state_machine() {
    let mock = MockRcServer::start().await;

    // Step 1: config/create asks for client_id
    mock.enqueue_wizard_step(serde_json::json!({
        "State": "client_id",
        "Option": {
            "Name": "client_id",
            "Help": "Google Application Client Id",
            "Type": "string",
            "Required": false,
            "Advanced": false
        },
        "Error": ""
    })).await;

    // Step 2: config/update answers client_id, rclone asks for client_secret
    mock.enqueue_wizard_step(serde_json::json!({
        "State": "client_secret",
        "Option": {
            "Name": "client_secret",
            "Help": "OAuth Client Secret",
            "Type": "string",
            "IsPassword": true,
            "Required": false,
            "Advanced": false
        },
        "Error": ""
    })).await;

    // Step 3: config/update answers client_secret, rclone completes
    mock.enqueue_wizard_step(serde_json::json!({
        "State": "",
        "Option": null,
        "Error": "",
        "Result": "OK"
    })).await;

    let client = RcClient::new(mock.base_url(), None, None);
    let mut driver = rcm_rc::WizardDriver::new(
        client,
        "my_gdrive",
        "drive",
        BTreeMap::new(),
        false,
    );

    // 1. Start wizard
    let step1 = driver.start().await.unwrap();
    match step1 {
        WizardStep::AskQuestion { state, option, error } => {
            assert_eq!(state, "client_id");
            assert_eq!(option.name, "client_id");
            assert!(error.is_none());
        }
        _ => panic!("Expected AskQuestion for step 1"),
    }

    // 2. Answer question 1
    let step2 = driver.answer("my_client_id.apps.googleusercontent.com", false).await.unwrap();
    match step2 {
        WizardStep::AskQuestion { state, option, error } => {
            assert_eq!(state, "client_secret");
            assert_eq!(option.name, "client_secret");
            assert!(option.is_password);
            assert!(error.is_none());
        }
        _ => panic!("Expected AskQuestion for step 2"),
    }

    // 3. Answer question 2
    let step3 = driver.answer("my_secret_token", true).await.unwrap();
    match step3 {
        WizardStep::Completed { remote_name } => {
            assert_eq!(remote_name, "my_gdrive");
        }
        _ => panic!("Expected Completed for step 3"),
    }

    mock.stop();
}

/// Traceability: FR-RM-10, FR-RM-12, FR-RM-14
/// Verifies OAuth status polling and cancellation flow
#[tokio::test]
async fn test_fr_rm_10_rm_12_rm_14_oauth_polling_and_cancellation() {
    let mock = MockRcServer::start().await;

    mock.set_route("config/oauthstatus", serde_json::json!({
        "AuthUrl": "http://127.0.0.1:53682/auth?state=xyz",
        "Status": "waiting",
        "Error": null
    })).await;
    mock.set_route("config/oauthstop", serde_json::json!({})).await;
    mock.set_route("config/delete", serde_json::json!({})).await;

    let client = RcClient::new(mock.base_url(), None, None);
    let driver = rcm_rc::WizardDriver::new(
        client,
        "oauth_remote",
        "drive",
        BTreeMap::new(),
        false,
    );

    let status = driver.poll_oauth().await.unwrap();
    assert_eq!(status.status, "waiting");
    assert_eq!(status.auth_url, Some("http://127.0.0.1:53682/auth?state=xyz".to_string()));

    let cancel_res = driver.cancel().await;
    assert!(cancel_res.is_ok());

    mock.stop();
}

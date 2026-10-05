use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use camino::Utf8PathBuf;
use rcm_core::{MutationIntent, SnapshotReason};
use rcm_rc::{MockRcServer, RcClient};
use rcm_config::ini::IniConfig;
use rcm_config::diff::diff_configs;
use rcm_config::mutation::MutationQueue;
use rcm_config::restore::RestoreManager;
use rcm_config::snapshot::{SnapshotRetention, SnapshotStore};
use rcm_config::watcher::ConfigWatcher;

const SAMPLE_CONF: &str = r#"
# Encrypted or normal rclone configuration file
[my_s3]
type = s3
provider = AWS
access_key_id = AKIAIOSFODNN7EXAMPLE
secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
region = us-east-1

[my_crypt]
type = crypt
remote = my_s3:secret_bucket
password = test_obscured_password_123
"#;

const SAMPLE_CONF_UPDATED: &str = r#"
[my_s3]
type = s3
provider = AWS
access_key_id = AKIAIOSFODNN7EXAMPLE
secret_access_key = changed_secret_key_new
region = us-west-2

[my_crypt]
type = crypt
remote = my_s3:secret_bucket
password = test_obscured_password_123
"#;

#[test]
fn test_ci_1_ini_parser_sections_and_keys() {
    let parsed = IniConfig::parse_str(SAMPLE_CONF).expect("Failed to parse ini");
    assert_eq!(parsed.sections.len(), 2);

    let s3 = parsed.section("my_s3").expect("my_s3 missing");
    assert_eq!(s3.get("type"), Some("s3"));
    assert_eq!(s3.get("region"), Some("us-east-1"));
    assert_eq!(s3.get("access_key_id"), Some("AKIAIOSFODNN7EXAMPLE"));

    let crypt = parsed.section("my_crypt").expect("my_crypt missing");
    assert_eq!(crypt.get("remote"), Some("my_s3:secret_bucket"));
}

#[test]
fn test_ci_5_bk_2_redacted_diff() {
    let conf_a = IniConfig::parse_str(SAMPLE_CONF).unwrap();
    let conf_b = IniConfig::parse_str(SAMPLE_CONF_UPDATED).unwrap();

    let diff = diff_configs(&conf_a, &conf_b);
    let rendered = diff.render_redacted();

    // Sensitive keys such as secret_access_key and password must be redacted
    assert!(!rendered.contains("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"));
    assert!(!rendered.contains("changed_secret_key_new"));
    assert!(rendered.contains("***REDACTED***"));

    // Non-sensitive diffs should be visible
    assert!(rendered.contains("us-east-1"));
    assert!(rendered.contains("us-west-2"));
}

#[tokio::test]
async fn test_bk_1_snapshot_store_dedupe_and_retention() {
    let temp_dir = Utf8PathBuf::from_path_buf(
        std::env::temp_dir().join(format!("rcm_test_snapshots_{}", uuid::Uuid::new_v4())),
    ).unwrap();
    let _ = std::fs::create_dir_all(&temp_dir);

    let store = SnapshotStore::new(&temp_dir);

    // Save initial snapshot
    let s1 = store
        .save_snapshot(SAMPLE_CONF.as_bytes(), SnapshotReason::FirstRun)
        .await
        .expect("Snapshot 1 failed");
    assert_eq!(s1.reason, SnapshotReason::FirstRun);
    assert_eq!(s1.remote_names, vec!["my_s3", "my_crypt"]);

    // Attempting to save identical content should deduplicate and return the existing snapshot
    let s2 = store
        .save_snapshot(SAMPLE_CONF.as_bytes(), SnapshotReason::PreMutation)
        .await
        .expect("Snapshot 2 dedupe failed");
    assert_eq!(s1.content_hash, s2.content_hash);

    let list = store.list_snapshots().await.expect("List failed");
    assert_eq!(list.len(), 1, "Duplicate snapshot content was not deduped");

    // Save updated content
    let s3 = store
        .save_snapshot(SAMPLE_CONF_UPDATED.as_bytes(), SnapshotReason::PreMutation)
        .await
        .expect("Snapshot 3 failed");
    assert_ne!(s1.content_hash, s3.content_hash);

    let list_updated = store.list_snapshots().await.expect("List updated failed");
    assert_eq!(list_updated.len(), 2);

    // Test retention pruning
    let retention = SnapshotRetention {
        max_pre_mutation: 1,
        max_daily: 1,
        max_weekly: 1,
    };
    store.prune_retention(&retention).await.expect("Prune failed");

    let list_pruned = store.list_snapshots().await.expect("List pruned failed");
    assert!(list_pruned.len() <= 2);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_ci_2_ci_3_ci_4_mutation_queue_transaction() {
    let mock = MockRcServer::start().await;

    // Set up mock routes for config/create, config/listremotes, fscache/clear
    mock.set_route("config/create", serde_json::json!({})).await;
    mock.set_route("config/listremotes", serde_json::json!({
        "remotes": ["new_remote"]
    })).await;
    mock.set_route("fscache/clear", serde_json::json!({})).await;

    let temp_snap_dir = Utf8PathBuf::from_path_buf(
        std::env::temp_dir().join(format!("rcm_test_mut_snap_{}", uuid::Uuid::new_v4())),
    ).unwrap();
    let snapshot_store = Arc::new(SnapshotStore::new(&temp_snap_dir));

    let client = RcClient::new(mock.base_url(), None, None);
    let is_restoring = Arc::new(AtomicBool::new(false));

    let queue = MutationQueue::new(
        client,
        snapshot_store.clone(),
        None,
        is_restoring.clone(),
    );

    // Test CI-4: Mutation is rejected when restore is in progress
    is_restoring.store(true, Ordering::SeqCst);
    let reject_res = queue.execute_mutation(MutationIntent::CreateRemote {
        name: "new_remote".to_string(),
        backend_type: "s3".to_string(),
        parameters: BTreeMap::new(),
        opt: BTreeMap::new(),
    }).await;
    assert!(reject_res.is_err());

    // When restore completes, mutation succeeds
    is_restoring.store(false, Ordering::SeqCst);
    let ok_res = queue.execute_mutation(MutationIntent::CreateRemote {
        name: "new_remote".to_string(),
        backend_type: "s3".to_string(),
        parameters: BTreeMap::new(),
        opt: BTreeMap::new(),
    }).await;
    assert!(ok_res.is_ok());

    mock.stop();
    let _ = std::fs::remove_dir_all(&temp_snap_dir);
}

#[tokio::test]
async fn test_cf_10_rename_remote_emulation() {
    let mock = MockRcServer::start().await;

    // Mock old remote
    mock.set_route("config/get", serde_json::json!({
        "type": "s3",
        "provider": "AWS",
        "region": "us-east-1"
    })).await;
    mock.set_route("config/create", serde_json::json!({})).await;
    mock.set_route("config/dump", serde_json::json!({
        "my_crypt": {
            "type": "crypt",
            "remote": "old_s3:vault"
        }
    })).await;
    mock.set_route("config/update", serde_json::json!({})).await;
    mock.set_route("config/delete", serde_json::json!({})).await;
    mock.set_route("fscache/clear", serde_json::json!({})).await;

    let temp_snap_dir = Utf8PathBuf::from_path_buf(
        std::env::temp_dir().join(format!("rcm_test_rename_snap_{}", uuid::Uuid::new_v4())),
    ).unwrap();
    let snapshot_store = Arc::new(SnapshotStore::new(&temp_snap_dir));
    let client = RcClient::new(mock.base_url(), None, None);
    let is_restoring = Arc::new(AtomicBool::new(false));

    let queue = MutationQueue::new(
        client,
        snapshot_store.clone(),
        None,
        is_restoring,
    );

    let res = queue.execute_mutation(MutationIntent::RenameRemote {
        old_name: "old_s3".to_string(),
        new_name: "renamed_s3".to_string(),
    }).await;

    assert!(res.is_ok());

    mock.stop();
    let _ = std::fs::remove_dir_all(&temp_snap_dir);
}

#[tokio::test]
async fn test_bk_2_restore_swap() {
    let temp_dir = Utf8PathBuf::from_path_buf(
        std::env::temp_dir().join(format!("rcm_test_restore_{}", uuid::Uuid::new_v4())),
    ).unwrap();
    let _ = std::fs::create_dir_all(&temp_dir);

    let snapshot_dir = temp_dir.join("snapshots");
    let store = SnapshotStore::new(&snapshot_dir);

    // Save snapshot 1
    let snap = store
        .save_snapshot(SAMPLE_CONF.as_bytes(), SnapshotReason::Manual)
        .await
        .unwrap();

    let config_file = temp_dir.join("rclone.conf");
    std::fs::write(&config_file, SAMPLE_CONF_UPDATED).unwrap();

    let restore_mgr = RestoreManager::new(store);
    restore_mgr
        .swap_restore(&snap.filename, &config_file)
        .await
        .expect("Swap restore failed");

    let restored_content = std::fs::read_to_string(&config_file).unwrap();
    assert!(restored_content.contains("us-east-1"));
    assert!(!restored_content.contains("us-west-2"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_cf_9_config_watcher() {
    let temp_dir = Utf8PathBuf::from_path_buf(
        std::env::temp_dir().join(format!("rcm_test_watcher_{}", uuid::Uuid::new_v4())),
    ).unwrap();
    let _ = std::fs::create_dir_all(&temp_dir);

    let config_file = temp_dir.join("rclone.conf");
    std::fs::write(&config_file, "initial").unwrap();

    let snap_dir = temp_dir.join("snapshots");
    let store = Arc::new(SnapshotStore::new(&snap_dir));

    let watcher = ConfigWatcher::new(config_file.clone(), store.clone());

    // First check computes initial hash
    let h1 = watcher.check_for_changes().await;
    assert!(h1.is_some());

    // Second check without file change yields None
    let h2 = watcher.check_for_changes().await;
    assert!(h2.is_none());

    // Update file
    std::fs::write(&config_file, "modified content").unwrap();

    // Next check detects change
    let h3 = watcher.check_for_changes().await;
    assert!(h3.is_some());
    assert_ne!(h1, h3);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

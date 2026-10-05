use std::collections::BTreeMap;
use rcm_core::{
    CoreError, DaemonState, JobOperation, JobProfile, MountPreset, MountProfile,
    MountTarget, MutationIntent, Remote, RemoteDependencyGraph, ServeProfile, ServeProtocol,
    SnapshotReason,
};

#[test]
fn test_cf_1_remote_creation_and_validation() {
    let valid_remote = Remote::new("my-remote", "s3");
    assert!(valid_remote.is_ok());

    let empty_name = Remote::new("", "s3");
    assert!(matches!(empty_name, Err(CoreError::InvalidRemoteName(_))));

    let invalid_chars = Remote::new("remote:bad", "s3");
    assert!(matches!(invalid_chars, Err(CoreError::InvalidRemoteName(_))));
}

#[test]
fn test_cf_6_remote_dependency_graph_referrers() {
    let base_remote = Remote::new("my_s3", "s3").unwrap();
    let mut crypt_remote = Remote::new("my_crypt", "crypt").unwrap();
    crypt_remote.parameters.insert(
        "remote".to_string(),
        serde_json::Value::String("my_s3:secret_bucket".to_string()),
    );

    let mut union_remote = Remote::new("my_union", "union").unwrap();
    union_remote.parameters.insert(
        "upstreams".to_string(),
        serde_json::Value::String("my_s3:path1 my_crypt:path2".to_string()),
    );

    let remotes = vec![&base_remote, &crypt_remote, &union_remote];
    let graph = RemoteDependencyGraph::build(remotes);

    let s3_referrers = graph.referrers_of("my_s3");
    assert!(s3_referrers.contains("my_crypt"));
    assert!(s3_referrers.contains("my_union"));

    let crypt_referrers = graph.referrers_of("my_crypt");
    assert!(crypt_referrers.contains("my_union"));
    assert!(!crypt_referrers.contains("my_s3"));
}

#[test]
fn test_mt_1_mt_2_mount_profile_presets_and_options() {
    let mut profile = MountProfile::new(
        "Media Drive",
        "my_gdrive:Media",
        MountTarget::parse_drive_letter('G').unwrap(),
    );

    assert_eq!(profile.preset, MountPreset::Balanced);
    assert_eq!(profile.options.get("vfs_cache_mode"), Some(&"writes".to_string()));

    profile.preset = MountPreset::Streaming;
    profile.options = profile.preset.default_options();
    assert_eq!(profile.options.get("vfs_cache_mode"), Some(&"full".to_string()));
    assert_eq!(profile.options.get("vfs_read_ahead"), Some(&"256M".to_string()));

    assert!(profile.validate().is_ok());
}

#[test]
fn test_mt_4_mount_profile_windows_network_mode_validation() {
    let mut profile = MountProfile::new(
        "Bad Combination",
        "remote:path",
        MountTarget::Folder("C:/mnt/folder".into()),
    );
    profile.windows_network_mode = true;
    let res = profile.validate();
    assert!(matches!(res, Err(CoreError::Validation(_))));
}

#[test]
fn test_sv_3_serve_profile_safe_bind_validation() {
    let loopback_serve = ServeProfile::new(
        "Local WebDAV",
        "my_remote:folder",
        ServeProtocol::Webdav,
        "127.0.0.1:8080",
    );
    assert!(loopback_serve.validate().is_ok());

    let mut lan_serve = ServeProfile::new(
        "LAN WebDAV",
        "my_remote:folder",
        ServeProtocol::Webdav,
        "0.0.0.0:8080",
    );
    // Non-loopback without auth must fail
    assert!(lan_serve.validate().is_err());

    // With auth it must succeed
    lan_serve.user = Some("admin".to_string());
    lan_serve.pass = Some("secret123".to_string());
    assert!(lan_serve.validate().is_ok());
}

#[test]
fn test_ci_2_ci_3_mutation_intent_roundtrip() {
    let mut params = BTreeMap::new();
    params.insert("type".to_string(), serde_json::Value::String("s3".to_string()));
    let intent = MutationIntent::CreateRemote {
        name: "test_s3".to_string(),
        backend_type: "s3".to_string(),
        parameters: params,
        opt: BTreeMap::new(),
    };

    let serialized = serde_json::to_string(&intent).unwrap();
    let deserialized: MutationIntent = serde_json::from_str(&serialized).unwrap();
    assert_eq!(intent, deserialized);
}

#[test]
fn test_bk_1_snapshot_reason_parsing() {
    assert_eq!(
        "pre-mutation".parse::<SnapshotReason>().unwrap(),
        SnapshotReason::PreMutation
    );
    assert_eq!(
        "scheduled".parse::<SnapshotReason>().unwrap(),
        SnapshotReason::Scheduled
    );
    assert_eq!(
        "external-change".parse::<SnapshotReason>().unwrap(),
        SnapshotReason::ExternalChange
    );
}

#[test]
fn test_dm_3_daemon_state_lifecycle() {
    let state = DaemonState::Starting;
    assert!(!state.is_ready());
    assert!(!state.is_stopped());

    let ready_state = DaemonState::Ready {
        execute_id: "exec-123".to_string(),
        version: "v1.75.1".to_string(),
        pid: 4567,
        addr: "http://127.0.0.1:5572".to_string(),
    };
    assert!(ready_state.is_ready());
}

#[test]
fn test_ot_2_job_profile_validation() {
    let valid_job = JobProfile::new(
        "Backup to Cloud",
        JobOperation::Sync,
        "C:/Local/Docs",
        "my_remote:DocsBackup",
    );
    assert!(valid_job.validate().is_ok());

    let invalid_job = JobProfile::new("", JobOperation::Sync, "", "my_remote:");
    assert!(invalid_job.validate().is_err());
}

use std::collections::BTreeMap;
use rcm_core::{
    CoreError, DaemonState, JobOperation, JobProfile, MountPreset, MountProfile,
    MountTarget, MutationIntent, Remote, RemoteDependencyGraph, ServeProfile, ServeProtocol,
    SnapshotReason,
};

/// Traceability: FR-RM-04
/// Validates remote naming restrictions (no empty, no ':', no '/', no '\')
#[test]
fn test_fr_rm_04_remote_name_validation() {
    let valid_remote = Remote::new("my-remote", "s3");
    assert!(valid_remote.is_ok());

    let empty_name = Remote::new("", "s3");
    assert!(matches!(empty_name, Err(CoreError::InvalidRemoteName(_))));

    let invalid_colon = Remote::new("remote:bad", "s3");
    assert!(matches!(invalid_colon, Err(CoreError::InvalidRemoteName(_))));

    let invalid_slash = Remote::new("remote/bad", "s3");
    assert!(matches!(invalid_slash, Err(CoreError::InvalidRemoteName(_))));

    let invalid_backslash = Remote::new("remote\\bad", "s3");
    assert!(matches!(invalid_backslash, Err(CoreError::InvalidRemoteName(_))));
}

/// Traceability: FR-RM-16
/// Tests remote dependency graph evaluation to discover upstreams and referrers
#[test]
fn test_fr_rm_16_remote_dependency_graph() {
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

/// Traceability: FR-MT-02, FR-MT-04
/// Tests mount profile presets and flat VFS options mapping
#[test]
fn test_fr_mt_02_mt_04_mount_presets_options() {
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

/// Traceability: FR-MT-05
/// Validates rejection of network mode combined with a folder mountpoint
#[test]
fn test_fr_mt_05_windows_network_mode_validation() {
    let mut profile = MountProfile::new(
        "Bad Combination",
        "remote:path",
        MountTarget::Folder("C:/mnt/folder".into()),
    );
    profile.windows_network_mode = true;
    let res = profile.validate();
    assert!(matches!(res, Err(CoreError::Validation(_))));
}

/// Traceability: FR-SV-03, FR-SV-04
/// Tests serve profile loopback default and non-loopback authentication enforcement
#[test]
fn test_fr_sv_03_sv_04_serve_bind_auth_validation() {
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

/// Traceability: FR-LC-07
/// Tests daemon state transitions and readiness helper methods
#[test]
fn test_fr_lc_07_daemon_state_lifecycle() {
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

/// Traceability: FR-BK-01
/// Tests roundtrip serialization of mutation intents
#[test]
fn test_fr_bk_01_mutation_intent_serialization() {
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

/// Traceability: FR-BK-04
/// Tests parsing of snapshot reasons
#[test]
fn test_fr_bk_04_snapshot_reason_parsing() {
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

/// Traceability: FR-FL-07
/// Tests job profile validation
#[test]
fn test_fr_fl_07_job_profile_validation() {
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

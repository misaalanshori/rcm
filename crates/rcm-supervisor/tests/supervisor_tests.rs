use std::time::{Duration, Instant};
use camino::Utf8PathBuf;
use rcm_core::{MountProfile, MountTarget, ServeProfile, ServeProtocol};
use rcm_supervisor::binary::BinaryManager;
use rcm_supervisor::health::CrashLoopBreaker;
use rcm_supervisor::logs::LogRingBuffer;
use rcm_supervisor::reconciler::{Reconciler, ReconcilerAction};

#[test]
fn test_bu_1_binary_manager_layout_and_version_floor() {
    let temp_dir = Utf8PathBuf::from_path_buf(
        std::env::temp_dir().join(format!("rcm_test_bin_mgr_{}", uuid::Uuid::new_v4())),
    ).unwrap();
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut mgr = BinaryManager::new(&temp_dir);

    // Initial state: no binary registered
    assert!(mgr.get_current_binary_path().is_none());

    // Register a valid version >= 1.73.5
    let v175 = "v1.75.1";
    let bin_path = mgr.register_version(v175).expect("Registration failed");
    assert!(bin_path.as_str().contains("1.75.1"));
    assert_eq!(mgr.current_version(), Some("v1.75.1"));

    // Check version floor (BU-4)
    assert!(mgr.is_version_safe("v1.75.1"));
    assert!(mgr.is_version_safe("v1.73.5"));
    assert!(!mgr.is_version_safe("v1.72.0"));
    assert!(!mgr.is_version_safe("v1.65.2"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_dm_4_crash_loop_breaker() {
    let mut breaker = CrashLoopBreaker::new(5, Duration::from_secs(120));

    let now = Instant::now();
    // 4 crashes in short succession: should still allow retry
    for i in 0..4 {
        assert!(breaker.record_crash(now + Duration::from_secs(i * 10)));
    }

    // 5th crash within 2 minutes: should trip the breaker
    let tripped = !breaker.record_crash(now + Duration::from_secs(45));
    assert!(tripped, "Breaker should have tripped after 5 crashes in 45 seconds");

    // Reset after successful run
    breaker.reset();
    assert!(breaker.record_crash(now + Duration::from_secs(60)));
}

#[test]
fn test_dm_9_log_ring_buffer_bounded() {
    let buffer = LogRingBuffer::new(5);

    for i in 1..=10 {
        buffer.push(format!("log message {}", i));
    }

    let entries = buffer.get_recent();
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0], "log message 6");
    assert_eq!(entries[4], "log message 10");
}

#[test]
fn test_reconciler_diff_and_actions() {
    let mount_profile = MountProfile::new(
        "Media",
        "gdrive:Media",
        MountTarget::parse_drive_letter('M').unwrap(),
    );
    let mut autostart_mount = mount_profile.clone();
    autostart_mount.autostart = true;

    let serve_profile = ServeProfile::new(
        "LocalWebdav",
        "s3:bucket",
        ServeProtocol::Webdav,
        "127.0.0.1:8080",
    );
    let mut autostart_serve = serve_profile.clone();
    autostart_serve.autostart = true;

    let desired_mounts = vec![autostart_mount];
    let desired_serves = vec![autostart_serve];

    // Scenario 1: Actual is empty -> should start both
    let actions = Reconciler::compute_actions(
        &desired_mounts,
        &desired_serves,
        &[], // no actual mounts
        &[], // no actual serves
    );

    assert_eq!(actions.len(), 2);
    assert!(actions.iter().any(|a| matches!(a, ReconcilerAction::StartMount(_))));
    assert!(actions.iter().any(|a| matches!(a, ReconcilerAction::StartServe(_))));

    // Scenario 2: Actual matches desired -> no action
    let actual_mounts = vec![rcm_rc::types::MountInfo {
        fs: "gdrive:Media".to_string(),
        mount_point: "M:".to_string(),
        mounted_on: None,
        mount_type: None,
    }];
    let actual_serves = vec![rcm_rc::types::ServeInfo {
        id: Some(1),
        protocol: "webdav".to_string(),
        fs: "s3:bucket".to_string(),
        addr: "127.0.0.1:8080".to_string(),
    }];

    let actions2 = Reconciler::compute_actions(
        &desired_mounts,
        &desired_serves,
        &actual_mounts,
        &actual_serves,
    );
    assert!(actions2.is_empty(), "Desired matches actual, no actions expected");

    // Scenario 3: Actual has unmanaged mount (MT-7) -> mark unmanaged
    let unmanaged_mount = rcm_rc::types::MountInfo {
        fs: "dropbox:personal".to_string(),
        mount_point: "Z:".to_string(),
        mounted_on: None,
        mount_type: None,
    };
    let actions3 = Reconciler::compute_actions(
        &desired_mounts,
        &desired_serves,
        &[actual_mounts[0].clone(), unmanaged_mount],
        &actual_serves,
    );
    assert_eq!(actions3.len(), 1);
    match &actions3[0] {
        ReconcilerAction::MarkUnmanagedMount(info) => {
            assert_eq!(info.mount_point, "Z:");
        }
        _ => panic!("Expected MarkUnmanagedMount"),
    }
}

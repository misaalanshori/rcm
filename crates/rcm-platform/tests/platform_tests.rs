use rcm_platform::paths::RcmPaths;
use rcm_platform::fs_driver::check_filesystem_driver;
use rcm_platform::drive_letters::get_available_drive_letters;
use rcm_platform::credential::{delete_credential, get_credential, set_credential};

#[test]
fn test_paths_layout_structure() {
    let paths = RcmPaths::resolve().expect("Failed to resolve RCM paths");

    assert!(!paths.config_dir().as_str().is_empty());
    assert!(!paths.data_dir().as_str().is_empty());
    assert!(!paths.backups_dir().as_str().is_empty());
    assert!(!paths.logs_dir().as_str().is_empty());

    let rclone_conf = paths.default_rclone_config_path();
    assert!(rclone_conf.as_str().ends_with("rclone.conf"));
}

#[test]
fn test_in_3_mt_4_filesystem_driver_check() {
    let status = check_filesystem_driver();
    if !status.is_installed {
        assert!(!status.remediation_hint.is_empty());
        #[cfg(windows)]
        assert!(status.remediation_hint.contains("winget install WinFsp.WinFsp"));
    }
}

#[cfg(windows)]
#[test]
fn test_mt_1_available_drive_letters() {
    let available = get_available_drive_letters();
    assert!(!available.contains(&'C'));
    assert!(!available.is_empty());
}

#[test]
fn test_cf_8_credential_keyring_roundtrip() {
    let test_key = format!("rcm_test_key_{}", uuid::Uuid::new_v4());
    let secret = "super_secure_vault_pass_9988!";

    // Store
    if set_credential(&test_key, secret).is_ok() {
        // Retrieve
        let retrieved = get_credential(&test_key).expect("Failed to get credential from keyring");
        assert_eq!(retrieved, secret);

        // Delete
        let _ = delete_credential(&test_key);
    }
}

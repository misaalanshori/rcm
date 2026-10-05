use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;
use tokio::time::sleep;
use rcm_rc::RcClient;

struct RcdProcess {
    child: Child,
    port: u16,
}

impl Drop for RcdProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn find_rclone_exe() -> Option<PathBuf> {
    let candidate = PathBuf::from("../../rman-scratchbox/rclone-v1.75.1-windows-amd64/rclone.exe");
    if candidate.exists() {
        return Some(candidate);
    }
    let candidate2 = PathBuf::from("../rman-scratchbox/rclone-v1.75.1-windows-amd64/rclone.exe");
    if candidate2.exists() {
        return Some(candidate2);
    }
    None
}

async fn spawn_temp_rcd() -> Option<RcdProcess> {
    let exe = find_rclone_exe()?;
    let port = 55720;
    let temp_dir = std::env::temp_dir().join(format!("rcm_test_rcd_{}", port));
    let _ = std::fs::create_dir_all(&temp_dir);
    let conf_path = temp_dir.join("rclone.conf");
    let _ = std::fs::write(&conf_path, "");

    let child = Command::new(exe)
        .arg("rcd")
        .arg("--rc-addr")
        .arg(format!("127.0.0.1:{}", port))
        .arg("--rc-no-auth")
        .arg("--config")
        .arg(&conf_path)
        .arg("--rc-job-expire-duration")
        .arg("1h")
        .spawn()
        .ok()?;

    let proc = RcdProcess { child, port };

    // Wait for rcd to come up
    let client = RcClient::new(format!("http://127.0.0.1:{}", port), None, None);
    let mut ready = false;
    for _ in 0..40 {
        sleep(Duration::from_millis(100)).await;
        if client.noop().await.is_ok() {
            ready = true;
            break;
        }
    }

    if ready {
        Some(proc)
    } else {
        None
    }
}

#[tokio::test]
async fn test_real_rclone_rc_integration() {
    let proc = match spawn_temp_rcd().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test_real_rclone_rc_integration: rclone binary not found or rcd failed to bind");
            return;
        }
    };

    let client = RcClient::new(format!("http://127.0.0.1:{}", proc.port), None, None);

    // 1. noop
    assert!(client.noop().await.is_ok());

    // 2. version
    let ver = client.version().await.unwrap();
    assert!(ver.version.contains("1.75"));

    // 3. obscure
    let obscured = client.obscure("supersecret").await.unwrap();
    assert_ne!(obscured, "supersecret");
    assert!(!obscured.is_empty());

    // 4. providers
    let providers = client.config_providers().await.unwrap();
    assert!(!providers.is_empty());
    let has_s3 = providers.iter().any(|p| p.prefix == "s3");
    let has_drive = providers.iter().any(|p| p.prefix == "drive");
    assert!(has_s3);
    assert!(has_drive);

    // 5. options/info
    let opts = client.options_info().await.unwrap();
    assert!(opts.groups.contains_key("vfs") || opts.groups.contains_key("main"));
}

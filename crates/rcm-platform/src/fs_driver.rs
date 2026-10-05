use camino::Utf8PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsDriverStatus {
    pub driver_name: String,
    pub is_installed: bool,
    pub version: Option<String>,
    pub install_path: Option<Utf8PathBuf>,
    pub remediation_hint: String,
}

/// Checks availability of user-mode filesystem driver (WinFsp on Windows, FUSE on Linux)
/// Satisfies MT-4 and IN-3
pub fn check_filesystem_driver() -> FsDriverStatus {
    #[cfg(windows)]
    {
        check_winfsp()
    }

    #[cfg(not(windows))]
    {
        check_fuse()
    }
}

#[cfg(windows)]
fn check_winfsp() -> FsDriverStatus {
    // 1. Check registry via reg query
    let reg_keys = [
        r"HKLM\SOFTWARE\WinFsp",
        r"HKLM\SOFTWARE\WOW6432Node\WinFsp",
    ];

    for key in reg_keys {
        if let Ok(output) = std::process::Command::new("reg")
            .args(["query", key, "/v", "InstallDir"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("InstallDir") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 {
                            let path_str = parts[2..].join(" ");
                            let install_path = Utf8PathBuf::from(path_str);
                            return FsDriverStatus {
                                driver_name: "WinFsp".to_string(),
                                is_installed: true,
                                version: None,
                                install_path: Some(install_path),
                                remediation_hint: String::new(),
                            };
                        }
                    }
                }
                return FsDriverStatus {
                    driver_name: "WinFsp".to_string(),
                    is_installed: true,
                    version: None,
                    install_path: None,
                    remediation_hint: String::new(),
                };
            }
        }
    }

    // 2. Check standard Program Files installation paths as fallback
    let fallback_paths = [
        r"C:\Program Files\WinFsp\bin\winfsp-x64.dll",
        r"C:\Program Files (x86)\WinFsp\bin\winfsp-x86.dll",
    ];

    for dll in fallback_paths {
        if std::path::Path::new(dll).exists() {
            let p = Utf8PathBuf::from(dll);
            return FsDriverStatus {
                driver_name: "WinFsp".to_string(),
                is_installed: true,
                version: None,
                install_path: p.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf()),
                remediation_hint: String::new(),
            };
        }
    }

    // Not installed
    FsDriverStatus {
        driver_name: "WinFsp".to_string(),
        is_installed: false,
        version: None,
        install_path: None,
        remediation_hint: "WinFsp is required to mount cloud remotes as Windows drive letters. Install via winget:\n  winget install WinFsp.WinFsp\nor download from https://winfsp.dev/".to_string(),
    }
}

#[cfg(not(windows))]
fn check_fuse() -> FsDriverStatus {
    // Check fusermount3 or fusermount in PATH
    let mut fuse_bin = None;
    for bin in ["fusermount3", "fusermount"] {
        if let Ok(output) = std::process::Command::new("which").arg(bin).output() {
            if output.status.success() {
                let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !out.is_empty() {
                    fuse_bin = Some(out);
                    break;
                }
            }
        }
    }

    let dev_fuse_exists = std::path::Path::new("/dev/fuse").exists();

    if let Some(bin_path) = fuse_bin {
        FsDriverStatus {
            driver_name: "FUSE".to_string(),
            is_installed: true,
            version: None,
            install_path: Some(Utf8PathBuf::from(bin_path)),
            remediation_hint: if !dev_fuse_exists {
                "Warning: /dev/fuse not found. Ensure the fuse kernel module is loaded ('modprobe fuse').".to_string()
            } else {
                String::new()
            },
        }
    } else {
        FsDriverStatus {
            driver_name: "FUSE".to_string(),
            is_installed: false,
            version: None,
            install_path: None,
            remediation_hint: "FUSE (fusermount3) is required to mount remotes on Linux. Install via your package manager (e.g., 'sudo apt install fuse3' or 'sudo dnf install fuse3').".to_string(),
        }
    }
}

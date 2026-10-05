use std::fs;
use std::process::Command;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use tracing::info;
use rcm_core::CoreError;

const MIN_SAFE_MAJOR: u32 = 1;
const MIN_SAFE_MINOR: u32 = 73;
const MIN_SAFE_PATCH: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryState {
    pub current: Option<String>,
    #[serde(default)]
    pub previous: Vec<String>,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default)]
    pub pinned: Option<String>,
}

fn default_channel() -> String {
    "stable".to_string()
}

impl Default for BinaryState {
    fn default() -> Self {
        Self {
            current: None,
            previous: Vec::new(),
            channel: default_channel(),
            pinned: None,
        }
    }
}

pub struct BinaryManager {
    base_dir: Utf8PathBuf,
    state_file: Utf8PathBuf,
    state: BinaryState,
}

impl BinaryManager {
    pub fn new(base_dir: &Utf8Path) -> Self {
        let state_dir = base_dir.join("state");
        let _ = fs::create_dir_all(&state_dir);
        let state_file = state_dir.join("binary.json");

        let state = if state_file.exists() {
            fs::read(&state_file)
                .ok()
                .and_then(|data| serde_json::from_slice(&data).ok())
                .unwrap_or_default()
        } else {
            BinaryState::default()
        };

        let mut mgr = Self {
            base_dir: base_dir.to_path_buf(),
            state_file,
            state,
        };

        // Scan for existing installed versions if none selected (BU-1, §7.9)
        let _ = mgr.scan_and_auto_adopt();

        mgr
    }

    pub fn current_version(&self) -> Option<&str> {
        self.state.current.as_deref()
    }

    pub fn is_version_safe(&self, ver: &str) -> bool {
        let clean = ver.trim().trim_start_matches('v');
        let parts: Vec<&str> = clean.split('.').collect();
        if parts.len() < 2 {
            return false;
        }

        let major: u32 = parts[0].parse().unwrap_or(0);
        let minor: u32 = parts[1].parse().unwrap_or(0);
        let patch: u32 = if parts.len() >= 3 {
            let p_str = parts[2].split('-').next().unwrap_or("0");
            p_str.parse().unwrap_or(0)
        } else {
            0
        };

        if major > MIN_SAFE_MAJOR {
            return true;
        }
        if major == MIN_SAFE_MAJOR {
            if minor > MIN_SAFE_MINOR {
                return true;
            }
            if minor == MIN_SAFE_MINOR && patch >= MIN_SAFE_PATCH {
                return true;
            }
        }
        false
    }

    pub fn get_current_binary_path(&self) -> Option<Utf8PathBuf> {
        let ver = self.state.current.as_ref()?;
        let path = self.version_binary_path(ver);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    pub fn version_binary_path(&self, version: &str) -> Utf8PathBuf {
        let exe_name = if cfg!(windows) { "rclone.exe" } else { "rclone" };
        self.base_dir.join("rclone").join(version).join(exe_name)
    }

    pub fn register_version(&mut self, version: &str) -> Result<Utf8PathBuf, CoreError> {
        let target_dir = self.base_dir.join("rclone").join(version);
        fs::create_dir_all(&target_dir).map_err(|e| {
            CoreError::Validation(format!("Failed to create version dir {}: {}", target_dir, e))
        })?;

        let bin_path = self.version_binary_path(version);

        if let Some(prev) = self.state.current.take() {
            if prev != version && !self.state.previous.contains(&prev) {
                self.state.previous.push(prev);
                if self.state.previous.len() > 2 {
                    self.state.previous.remove(0);
                }
            }
        }

        self.state.current = Some(version.to_string());
        self.save_state()?;

        Ok(bin_path)
    }

    pub fn rollback(&mut self) -> Result<Option<String>, CoreError> {
        if let Some(prev) = self.state.previous.pop() {
            self.state.current = Some(prev.clone());
            self.save_state()?;
            Ok(Some(prev))
        } else {
            Ok(None)
        }
    }

    /// Scans `<base_dir>/rclone/` for already-installed rclone versions and sets the newest safe one as current
    pub fn scan_and_auto_adopt(&mut self) -> Result<Option<String>, CoreError> {
        let rclone_root = self.base_dir.join("rclone");
        if !rclone_root.exists() {
            return Ok(None);
        }

        let entries = match fs::read_dir(&rclone_root) {
            Ok(e) => e,
            Err(_) => return Ok(None),
        };

        let mut found_versions = Vec::new();

        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    let bin_path = self.version_binary_path(&dir_name);
                    if bin_path.exists() && self.is_version_safe(&dir_name) {
                        found_versions.push(dir_name);
                    }
                }
            }
        }

        if found_versions.is_empty() {
            return Ok(None);
        }

        // Sort descending to find the newest
        found_versions.sort();
        let newest = found_versions.last().cloned();

        if let Some(ref ver) = newest {
            if self.state.current.as_ref() != Some(ver) {
                self.state.current = Some(ver.clone());
                self.save_state()?;
            }
        }

        Ok(newest)
    }

    /// Registers an existing rclone executable (BU-6), copies it into managed directory, and sets it as current
    pub fn register_existing_binary(&mut self, source_path: &Utf8Path) -> Result<String, CoreError> {
        if !source_path.exists() {
            return Err(CoreError::NotFound(format!(
                "rclone binary not found at '{}'",
                source_path
            )));
        }

        let output = Command::new(source_path.as_std_path())
            .arg("version")
            .output()
            .map_err(|e| {
                CoreError::Validation(format!(
                    "Failed to execute '{} version': {}",
                    source_path, e
                ))
            })?;

        if !output.status.success() {
            return Err(CoreError::Validation(format!(
                "rclone at '{}' exited with non-zero status on version check",
                source_path
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next().unwrap_or("").trim();
        let ver = if let Some(pos) = first_line.find('v') {
            first_line[pos..].split_whitespace().next().unwrap_or("").to_string()
        } else {
            return Err(CoreError::Validation(format!(
                "Could not parse version from rclone output: '{}'",
                first_line
            )));
        };

        if !self.is_version_safe(&ver) {
            return Err(CoreError::Validation(format!(
                "rclone version '{}' is below minimum safe version floor (v1.73.5) per BU-4",
                ver
            )));
        }

        let target_path = self.register_version(&ver)?;
        fs::copy(source_path.as_std_path(), target_path.as_std_path()).map_err(|e| {
            CoreError::Validation(format!(
                "Failed to copy rclone binary from '{}' to '{}': {}",
                source_path, target_path, e
            ))
        })?;

        info!("Registered rclone version {} from {}", ver, source_path);
        Ok(ver)
    }

    /// Fetches and installs the latest stable rclone directly from official release downloads (BU-1, BU-2, BU-3)
    pub fn fetch_and_install_rclone(
        &mut self,
        target_version: Option<&str>,
    ) -> Result<String, CoreError> {
        let version_tag = match target_version {
            Some(v) => {
                if v.starts_with('v') {
                    v.to_string()
                } else {
                    format!("v{}", v)
                }
            }
            None => {
                // Fetch latest version from version.txt
                let out = Command::new("curl.exe")
                    .args(["-s", "https://downloads.rclone.org/version.txt"])
                    .output()
                    .or_else(|_| {
                        Command::new("curl")
                            .args(["-s", "https://downloads.rclone.org/version.txt"])
                            .output()
                    })
                    .map_err(|e| {
                        CoreError::Validation(format!(
                            "Failed to fetch latest rclone version metadata: {}",
                            e
                        ))
                    })?;

                let ver_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if ver_str.is_empty() {
                    return Err(CoreError::Validation(
                        "Empty response from rclone version endpoint".to_string(),
                    ));
                }
                // e.g. "rclone v1.75.1" -> "v1.75.1"
                if let Some(pos) = ver_str.find('v') {
                    ver_str[pos..].split_whitespace().next().unwrap_or("").to_string()
                } else {
                    ver_str
                }
            }
        };

        if !self.is_version_safe(&version_tag) {
            return Err(CoreError::Validation(format!(
                "Target version '{}' is below safe floor (v1.73.5)",
                version_tag
            )));
        }

        // Determine OS and architecture
        let (os_str, arch_str) = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", "x86_64") => ("windows", "amd64"),
            ("windows", "aarch64") => ("windows", "arm64"),
            ("linux", "x86_64") => ("linux", "amd64"),
            ("linux", "aarch64") => ("linux", "arm64"),
            (os, arch) => {
                return Err(CoreError::Validation(format!(
                    "Unsupported platform for automatic rclone fetch: {}/{}",
                    os, arch
                )));
            }
        };

        let archive_base = format!("rclone-{}-{}-{}", version_tag, os_str, arch_str);
        let archive_name = format!("{}.zip", archive_base);
        let download_url = format!("https://downloads.rclone.org/{}/{}", version_tag, archive_name);

        let temp_dir = std::env::temp_dir().join(format!("rcm_rclone_dl_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);
        let zip_path = temp_dir.join(&archive_name);

        info!("Downloading rclone {} from {}...", version_tag, download_url);

        // Download via curl
        let curl_status = Command::new("curl.exe")
            .args(["-f", "-L", "-o", zip_path.to_string_lossy().as_ref(), &download_url])
            .status()
            .or_else(|_| {
                Command::new("curl")
                    .args(["-f", "-L", "-o", zip_path.to_string_lossy().as_ref(), &download_url])
                    .status()
            })
            .map_err(|e| {
                let _ = fs::remove_dir_all(&temp_dir);
                CoreError::Validation(format!("Failed to run curl for download: {}", e))
            })?;

        if !curl_status.success() {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(CoreError::Validation(format!(
                "Download failed for {} (HTTP error)",
                download_url
            )));
        }

        // Extract zip
        let extract_dir = temp_dir.join("extracted");
        let _ = fs::create_dir_all(&extract_dir);

        let tar_status = Command::new("tar.exe")
            .args(["-xf", zip_path.to_string_lossy().as_ref(), "-C", extract_dir.to_string_lossy().as_ref()])
            .status()
            .or_else(|_| {
                Command::new("tar")
                    .args(["-xf", zip_path.to_string_lossy().as_ref(), "-C", extract_dir.to_string_lossy().as_ref()])
                    .status()
            })
            .or_else(|_| {
                // Fallback to PowerShell Expand-Archive
                Command::new("powershell.exe")
                    .args([
                        "-NoProfile",
                        "-Command",
                        &format!(
                            "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                            zip_path.to_string_lossy(),
                            extract_dir.to_string_lossy()
                        ),
                    ])
                    .status()
            });

        match tar_status {
            Ok(s) if s.success() => {}
            _ => {
                let _ = fs::remove_dir_all(&temp_dir);
                return Err(CoreError::Validation("Failed to extract rclone archive".to_string()));
            }
        }

        // Locate binary inside extracted directory
        let exe_name = if cfg!(windows) { "rclone.exe" } else { "rclone" };
        let candidate_path = extract_dir.join(&archive_base).join(exe_name);
        let extracted_bin = if candidate_path.exists() {
            candidate_path
        } else {
            // Scan for exe_name in extract_dir
            let mut found = None;
            for entry in fs::read_dir(&extract_dir).into_iter().flatten().flatten() {
                let p = entry.path().join(exe_name);
                if p.exists() {
                    found = Some(p);
                    break;
                }
            }
            match found {
                Some(p) => p,
                None => {
                    let _ = fs::remove_dir_all(&temp_dir);
                    return Err(CoreError::Validation(format!(
                        "Extracted archive did not contain '{}'",
                        exe_name
                    )));
                }
            }
        };

        // Register version and copy binary
        let target_path = self.register_version(&version_tag)?;
        fs::copy(&extracted_bin, target_path.as_std_path()).map_err(|e| {
            let _ = fs::remove_dir_all(&temp_dir);
            CoreError::Validation(format!("Failed to copy binary to managed location: {}", e))
        })?;

        // Clean up temp
        let _ = fs::remove_dir_all(&temp_dir);

        info!("Successfully installed and registered rclone {}", version_tag);
        Ok(version_tag)
    }

    fn save_state(&self) -> Result<(), CoreError> {
        let data = serde_json::to_vec_pretty(&self.state).map_err(|e| {
            CoreError::Serialization(format!("Failed to serialize binary state: {}", e))
        })?;
        fs::write(&self.state_file, data).map_err(|e| {
            CoreError::Validation(format!("Failed to write binary state file: {}", e))
        })?;
        Ok(())
    }
}

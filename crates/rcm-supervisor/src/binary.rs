use std::fs;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
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

        Self {
            base_dir: base_dir.to_path_buf(),
            state_file,
            state,
        }
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
            // strip any suffix like -beta
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

        // Update state
        if let Some(prev) = self.state.current.take() {
            if prev != version && !self.state.previous.contains(&prev) {
                self.state.previous.push(prev);
                // Keep max 2 previous versions per BU-3
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

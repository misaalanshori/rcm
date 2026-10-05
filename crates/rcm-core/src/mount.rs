use std::collections::BTreeMap;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use crate::error::CoreError;
use crate::id::Id;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum MountTarget {
    #[serde(rename = "drive_letter")]
    DriveLetter(char),
    #[serde(rename = "auto")]
    AutoDriveLetter,
    #[serde(rename = "folder")]
    Folder(Utf8PathBuf),
    #[serde(rename = "unc")]
    Unc(String),
}

impl MountTarget {
    pub fn parse_drive_letter(c: char) -> Result<Self, CoreError> {
        if !c.is_ascii_alphabetic() {
            return Err(CoreError::InvalidDriveLetter(c));
        }
        Ok(Self::DriveLetter(c.to_ascii_uppercase()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MountPreset {
    #[default]
    Balanced,
    Streaming,
    OfflineFirst,
    MaxCompatibility,
    ReadOnly,
    Custom,
}

impl MountPreset {
    /// Maps preset to standard recommended VFS & mount flags per SRDD §7.7
    pub fn default_options(&self) -> BTreeMap<String, String> {
        let mut opts = BTreeMap::new();
        match self {
            MountPreset::Balanced => {
                opts.insert("vfs_cache_mode".to_string(), "writes".to_string());
                opts.insert("dir_cache_time".to_string(), "30m".to_string());
            }
            MountPreset::Streaming => {
                opts.insert("vfs_cache_mode".to_string(), "full".to_string());
                opts.insert("vfs_read_ahead".to_string(), "256M".to_string());
                opts.insert("vfs_cache_max_age".to_string(), "6h".to_string());
                opts.insert("dir_cache_time".to_string(), "30m".to_string());
            }
            MountPreset::OfflineFirst => {
                opts.insert("vfs_cache_mode".to_string(), "full".to_string());
                opts.insert("vfs_cache_max_size".to_string(), "50G".to_string());
                opts.insert("vfs_cache_max_age".to_string(), "720h".to_string());
                opts.insert("dir_cache_time".to_string(), "24h".to_string());
            }
            MountPreset::MaxCompatibility => {
                opts.insert("vfs_cache_mode".to_string(), "full".to_string());
                opts.insert("no_checksum".to_string(), "true".to_string());
                opts.insert("no_modtime".to_string(), "true".to_string());
            }
            MountPreset::ReadOnly => {
                opts.insert("read_only".to_string(), "true".to_string());
                opts.insert("dir_cache_time".to_string(), "1h".to_string());
            }
            MountPreset::Custom => {}
        }
        opts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountProfile {
    pub id: Id,
    pub name: String,
    pub remote: String,
    pub target: MountTarget,
    #[serde(default = "default_mount_type")]
    pub mount_type: String,
    #[serde(default)]
    pub windows_network_mode: bool,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub preset: MountPreset,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    #[serde(default)]
    pub extra_flags: Vec<String>,
}

fn default_mount_type() -> String {
    "mount".to_string()
}

impl MountProfile {
    pub fn new(
        name: impl Into<String>,
        remote: impl Into<String>,
        target: MountTarget,
    ) -> Self {
        let preset = MountPreset::Balanced;
        let options = preset.default_options();
        Self {
            id: Id::new(),
            name: name.into(),
            remote: remote.into(),
            target,
            mount_type: default_mount_type(),
            windows_network_mode: false,
            autostart: false,
            preset,
            options,
            extra_flags: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::Validation("Mount name cannot be empty".to_string()));
        }
        if self.remote.trim().is_empty() {
            return Err(CoreError::Validation("Mount remote cannot be empty".to_string()));
        }
        // Validate Windows network mode requirement (MT-4: UNC requires network mode / drive letter, folder + network mode is invalid)
        if self.windows_network_mode {
            if let MountTarget::Folder(_) = &self.target {
                return Err(CoreError::Validation(
                    "Network mode cannot be combined with a folder mountpoint on Windows".to_string(),
                ));
            }
        }
        Ok(())
    }
}

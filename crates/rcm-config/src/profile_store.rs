use std::fs;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use rcm_core::{CoreError, Id, MountProfile, ServeProfile};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProfilesDocument {
    #[serde(default = "default_schema_version")]
    pub schema: u32,
    #[serde(default)]
    pub mount: Vec<MountProfile>,
    #[serde(default)]
    pub serve: Vec<ServeProfile>,
}

fn default_schema_version() -> u32 {
    1
}

pub struct ProfileStore {
    path: Utf8PathBuf,
    doc: ProfilesDocument,
}

impl ProfileStore {
    pub fn load_or_create(path: &Utf8Path) -> Result<Self, CoreError> {
        let doc = if path.exists() {
            let content = fs::read_to_string(path).map_err(|e| {
                CoreError::Validation(format!("Failed to read profiles at {}: {}", path, e))
            })?;
            toml::from_str(&content).map_err(|e| {
                CoreError::Serialization(format!("Corrupt profiles.toml at {}: {}", path, e))
            })?
        } else {
            ProfilesDocument::default()
        };

        Ok(Self {
            path: path.to_path_buf(),
            doc,
        })
    }

    pub fn list_mounts(&self) -> &[MountProfile] {
        &self.doc.mount
    }

    pub fn save_mount(&mut self, profile: MountProfile) -> Result<(), CoreError> {
        profile.validate()?;
        if let Some(pos) = self.doc.mount.iter().position(|m| m.id == profile.id) {
            self.doc.mount[pos] = profile;
        } else {
            self.doc.mount.push(profile);
        }
        self.flush()
    }

    pub fn delete_mount(&mut self, id: &Id) -> Result<(), CoreError> {
        self.doc.mount.retain(|m| &m.id != id);
        self.flush()
    }

    pub fn list_serves(&self) -> &[ServeProfile] {
        &self.doc.serve
    }

    pub fn save_serve(&mut self, profile: ServeProfile) -> Result<(), CoreError> {
        profile.validate()?;
        if let Some(pos) = self.doc.serve.iter().position(|s| s.id == profile.id) {
            self.doc.serve[pos] = profile;
        } else {
            self.doc.serve.push(profile);
        }
        self.flush()
    }

    pub fn delete_serve(&mut self, id: &Id) -> Result<(), CoreError> {
        self.doc.serve.retain(|s| &s.id != id);
        self.flush()
    }

    fn flush(&self) -> Result<(), CoreError> {
        let content = toml::to_string_pretty(&self.doc).map_err(|e| {
            CoreError::Serialization(format!("Failed to serialize profiles: {}", e))
        })?;
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&self.path, content).map_err(|e| {
            CoreError::Validation(format!("Failed to write profiles to {}: {}", self.path, e))
        })
    }
}

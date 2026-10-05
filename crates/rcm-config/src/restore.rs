use camino::Utf8Path;
use tokio::fs;
use rcm_core::{CoreError, SnapshotReason};
use rcm_rc::RcClient;
use crate::ini::IniConfig;
use crate::snapshot::SnapshotStore;

pub struct RestoreManager {
    snapshot_store: SnapshotStore,
}

impl RestoreManager {
    pub fn new(snapshot_store: SnapshotStore) -> Self {
        Self { snapshot_store }
    }

    /// Restores a full configuration file via atomic swap while rcd is stopped (BK-2, §7.5)
    pub async fn swap_restore(
        &self,
        snapshot_filename: &str,
        target_config_path: &Utf8Path,
    ) -> Result<(), CoreError> {
        // Step 1: Take pre-restore snapshot of current config if it exists
        if target_config_path.exists() {
            let current_content = fs::read(target_config_path).await.map_err(|e| {
                CoreError::Validation(format!("Failed to read config for pre-restore snapshot: {}", e))
            })?;
            let _ = self
                .snapshot_store
                .save_snapshot(&current_content, SnapshotReason::PreRestore)
                .await?;
        }

        // Step 2: Read snapshot content
        let snapshot_bytes = self
            .snapshot_store
            .read_snapshot_content(snapshot_filename)
            .await?;

        // Step 3: Write to temporary file in the same directory for atomic rename
        let parent_dir = target_config_path.parent().unwrap_or(target_config_path);
        let temp_path = parent_dir.join(format!(
            ".rclone.conf.restore.{}.tmp",
            uuid::Uuid::new_v4()
        ));

        fs::write(&temp_path, &snapshot_bytes).await.map_err(|e| {
            CoreError::Validation(format!("Failed to write restore temp file: {}", e))
        })?;

        // Step 4: Atomic swap (rename over target)
        // On Windows std::fs::rename will fail if target exists without remove, so we remove then rename or use std::fs
        if target_config_path.exists() {
            let _ = fs::remove_file(target_config_path).await;
        }

        if let Err(e) = fs::rename(&temp_path, target_config_path).await {
            // Clean up temp file on failure
            let _ = fs::remove_file(&temp_path).await;
            return Err(CoreError::Validation(format!(
                "Failed to replace configuration file during restore: {}",
                e
            )));
        }

        Ok(())
    }

    /// Selective restore of a single remote from a plaintext snapshot (BK-2)
    pub async fn selective_restore_remote(
        &self,
        snapshot_filename: &str,
        remote_name: &str,
        rc_client: &RcClient,
    ) -> Result<(), CoreError> {
        let snapshot_bytes = self
            .snapshot_store
            .read_snapshot_content(snapshot_filename)
            .await?;
        let content_str = std::str::from_utf8(&snapshot_bytes).map_err(|_| {
            CoreError::Validation("Cannot selectively restore an encrypted or non-UTF8 snapshot".to_string())
        })?;

        let ini = IniConfig::parse_str(content_str)?;
        let sec = ini.section(remote_name).ok_or_else(|| {
            CoreError::NotFound(format!(
                "Remote '{}' not found in snapshot {}",
                remote_name, snapshot_filename
            ))
        })?;

        let backend_type = sec.get("type").unwrap_or_default().to_string();
        let mut params = std::collections::BTreeMap::new();
        for (k, v) in &sec.properties {
            if k != "type" {
                params.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
        }

        let mut opt = std::collections::BTreeMap::new();
        opt.insert("noObscure".to_string(), serde_json::Value::Bool(true));
        opt.insert("nonInteractive".to_string(), serde_json::Value::Bool(true));

        rc_client
            .config_create(remote_name, &backend_type, params, opt)
            .await
            .map_err(|e| CoreError::Validation(format!("Selective restore failed: {}", e)))?;

        let _ = rc_client.fscache_clear().await;

        Ok(())
    }
}

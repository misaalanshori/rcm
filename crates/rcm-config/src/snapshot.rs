use std::time::{SystemTime, UNIX_EPOCH};
use camino::{Utf8Path, Utf8PathBuf};
use sha2::{Digest, Sha256};
use tokio::fs;
use rcm_core::{CoreError, Id, SnapshotMeta, SnapshotReason};
use crate::ini::IniConfig;

#[derive(Debug, Clone)]
pub struct SnapshotRetention {
    pub max_pre_mutation: usize,
    pub max_daily: usize,
    pub max_weekly: usize,
}

impl Default for SnapshotRetention {
    fn default() -> Self {
        Self {
            max_pre_mutation: 50,
            max_daily: 30,
            max_weekly: 12,
        }
    }
}

pub struct SnapshotStore {
    dir: Utf8PathBuf,
    index_path: Utf8PathBuf,
}

impl SnapshotStore {
    pub fn new(dir: &Utf8Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            index_path: dir.join("index.json"),
        }
    }

    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotMeta>, CoreError> {
        self.read_index().await
    }

    /// Saves a snapshot of rclone.conf content with content-hash deduplication (BK-1)
    pub async fn save_snapshot(
        &self,
        content: &[u8],
        reason: SnapshotReason,
    ) -> Result<SnapshotMeta, CoreError> {
        fs::create_dir_all(&self.dir).await.map_err(|e| {
            CoreError::Validation(format!("Failed to create snapshot dir: {}", e))
        })?;

        let mut hasher = Sha256::new();
        hasher.update(content);
        let hash_bytes = hasher.finalize();
        let hash_hex = format!("{:x}", hash_bytes);
        let hash8 = &hash_hex[..8];

        let mut existing_snapshots = self.read_index().await?;

        // Content deduplication check: if newest snapshot matches hash, return it without creating a duplicate
        if let Some(newest) = existing_snapshots.last() {
            if newest.content_hash == hash_hex {
                return Ok(newest.clone());
            }
        }

        let now_utc = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let filename = format!("rclone.conf.{}.{}.{}", now_utc, reason, hash8);
        let target_path = self.dir.join(&filename);

        // Write snapshot file
        fs::write(&target_path, content).await.map_err(|e| {
            CoreError::Validation(format!("Failed to write snapshot {}: {}", filename, e))
        })?;

        // Inspect content (best-effort parse to extract remote names and check encryption header)
        let is_encrypted = content.starts_with(b"# Encrypted rclone configuration File")
            || content.starts_with(b"RCLONE_ENCRYPTED");

        let remote_names = if !is_encrypted {
            if let Ok(content_str) = std::str::from_utf8(content) {
                IniConfig::parse_str(content_str)
                    .map(|c| c.remote_names())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let meta = SnapshotMeta {
            id: Id::new(),
            filename,
            timestamp_utc: now_utc,
            reason,
            content_hash: hash_hex,
            file_size_bytes: content.len() as u64,
            remote_names,
            is_encrypted,
        };

        existing_snapshots.push(meta.clone());
        self.write_index(&existing_snapshots).await?;

        Ok(meta)
    }

    pub async fn read_snapshot_content(&self, filename: &str) -> Result<Vec<u8>, CoreError> {
        let path = self.dir.join(filename);
        fs::read(&path).await.map_err(|e| {
            CoreError::NotFound(format!("Snapshot file {} not found: {}", filename, e))
        })
    }

    pub async fn prune_retention(&self, retention: &SnapshotRetention) -> Result<(), CoreError> {
        let mut snapshots = self.read_index().await?;
        if snapshots.is_empty() {
            return Ok(());
        }

        // Separate pre-mutation vs scheduled/others
        let mut pre_mutations = Vec::new();
        let mut others = Vec::new();

        for s in snapshots.drain(..) {
            if s.reason == SnapshotReason::PreMutation {
                pre_mutations.push(s);
            } else {
                others.push(s);
            }
        }

        let mut to_keep = Vec::new();
        let mut to_delete = Vec::new();

        // Keep last N pre-mutation snapshots
        if pre_mutations.len() > retention.max_pre_mutation {
            let split_at = pre_mutations.len() - retention.max_pre_mutation;
            to_delete.extend(pre_mutations.drain(..split_at));
        }
        to_keep.extend(pre_mutations);

        // Keep last N others
        let max_others = retention.max_daily + retention.max_weekly;
        if others.len() > max_others {
            let split_at = others.len() - max_others;
            to_delete.extend(others.drain(..split_at));
        }
        to_keep.extend(others);

        // Delete pruned files
        for d in to_delete {
            let path = self.dir.join(&d.filename);
            let _ = fs::remove_file(path).await;
        }

        to_keep.sort_by_key(|s| s.timestamp_utc);
        self.write_index(&to_keep).await?;

        Ok(())
    }

    async fn read_index(&self) -> Result<Vec<SnapshotMeta>, CoreError> {
        if !self.index_path.exists() {
            return Ok(Vec::new());
        }
        let data = fs::read(&self.index_path).await.map_err(|e| {
            CoreError::Validation(format!("Failed to read index: {}", e))
        })?;
        serde_json::from_slice(&data).map_err(|e| {
            CoreError::Serialization(format!("Corrupt index.json: {}", e))
        })
    }

    async fn write_index(&self, index: &[SnapshotMeta]) -> Result<(), CoreError> {
        let data = serde_json::to_vec_pretty(index).map_err(|e| {
            CoreError::Serialization(format!("Failed to serialize index: {}", e))
        })?;
        fs::write(&self.index_path, data).await.map_err(|e| {
            CoreError::Validation(format!("Failed to write index.json: {}", e))
        })
    }
}

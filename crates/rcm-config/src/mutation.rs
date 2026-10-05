use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::fs;
use camino::Utf8PathBuf;
use rcm_core::{CoreError, MutationIntent, Remote, RemoteDependencyGraph, SnapshotReason};
use rcm_rc::RcClient;
use crate::snapshot::SnapshotStore;

pub struct MutationQueue {
    lock: Mutex<()>,
    rc_client: RcClient,
    snapshot_store: Arc<SnapshotStore>,
    config_file_path: Option<Utf8PathBuf>,
    is_restoring: Arc<AtomicBool>,
}

impl MutationQueue {
    pub fn new(
        rc_client: RcClient,
        snapshot_store: Arc<SnapshotStore>,
        config_file_path: Option<Utf8PathBuf>,
        is_restoring: Arc<AtomicBool>,
    ) -> Self {
        Self {
            lock: Mutex::new(()),
            rc_client,
            snapshot_store,
            config_file_path,
            is_restoring,
        }
    }

    /// Sets or updates the config file path
    pub fn set_config_file_path(&mut self, path: Utf8PathBuf) {
        self.config_file_path = Some(path);
    }

    /// Executes a mutation intent through the transactional pipeline (CI-2, CI-3, CI-4)
    pub async fn execute_mutation(&self, intent: MutationIntent) -> Result<(), CoreError> {
        // CI-4: Mutations refused while a restore is running
        if self.is_restoring.load(Ordering::SeqCst) {
            return Err(CoreError::Validation(
                "Cannot perform config mutation while a configuration restore is in progress".to_string(),
            ));
        }

        // CI-2: One mutation in flight at a time
        let _guard = self.lock.lock().await;

        // Step 1: Pre-mutation snapshot (BK-1, CI-3)
        self.take_pre_mutation_snapshot_if_needed().await?;

        // Step 2: Execute mutation via RC
        match intent {
            MutationIntent::CreateRemote {
                name,
                backend_type,
                parameters,
                opt,
            } => {
                let mut opt_json = BTreeMap::new();
                for (k, v) in opt {
                    opt_json.insert(k, serde_json::Value::String(v));
                }
                self.rc_client
                    .config_create(&name, &backend_type, parameters, opt_json)
                    .await
                    .map_err(|e| CoreError::Validation(format!("RC config/create failed: {}", e)))?;

                // Verification (CI-3)
                let remotes = self
                    .rc_client
                    .config_list_remotes()
                    .await
                    .map_err(|e| CoreError::Validation(format!("Verification failed: {}", e)))?;
                if !remotes.contains(&name) {
                    return Err(CoreError::Validation(format!(
                        "Verification failed: remote '{}' was not created",
                        name
                    )));
                }
            }

            MutationIntent::UpdateRemote {
                name,
                parameters,
                opt,
            } => {
                let mut opt_json = BTreeMap::new();
                for (k, v) in opt {
                    opt_json.insert(k, serde_json::Value::String(v));
                }
                self.rc_client
                    .config_update(&name, parameters, opt_json)
                    .await
                    .map_err(|e| CoreError::Validation(format!("RC config/update failed: {}", e)))?;

                // Verification
                let _ = self
                    .rc_client
                    .config_get(&name)
                    .await
                    .map_err(|e| CoreError::Validation(format!("Verification failed: {}", e)))?;
            }

            MutationIntent::DeleteRemote { name } => {
                self.rc_client
                    .config_delete(&name)
                    .await
                    .map_err(|e| CoreError::Validation(format!("RC config/delete failed: {}", e)))?;

                // Verification
                let remotes = self
                    .rc_client
                    .config_list_remotes()
                    .await
                    .map_err(|e| CoreError::Validation(format!("Verification failed: {}", e)))?;
                if remotes.contains(&name) {
                    return Err(CoreError::Validation(format!(
                        "Verification failed: remote '{}' was not deleted",
                        name
                    )));
                }
            }

            MutationIntent::DuplicateRemote {
                source_name,
                new_name,
            } => {
                self.duplicate_remote_emulation(&source_name, &new_name)
                    .await?;
            }

            MutationIntent::RenameRemote {
                old_name,
                new_name,
            } => {
                self.rename_remote_emulation(&old_name, &new_name).await?;
            }

            MutationIntent::UnlockConfig {
                password_keyring_key,
            } => {
                self.rc_client
                    .config_unlock(&password_keyring_key)
                    .await
                    .map_err(|e| CoreError::Validation(format!("RC config/unlock failed: {}", e)))?;
            }

            MutationIntent::SetConfigPath { path } => {
                self.rc_client
                    .config_set_path(path.as_str())
                    .await
                    .map_err(|e| CoreError::Validation(format!("RC config/setpath failed: {}", e)))?;
            }
        }

        // Step 3: Clear fscache (R11, CI-3)
        let _ = self.rc_client.fscache_clear().await;

        Ok(())
    }

    /// Emulates duplicate remote via RC (CF-10)
    async fn duplicate_remote_emulation(
        &self,
        source_name: &str,
        new_name: &str,
    ) -> Result<(), CoreError> {
        let mut source_params = self.rc_client.config_get(source_name).await.map_err(|e| {
            CoreError::NotFound(format!("Source remote '{}' not found: {}", source_name, e))
        })?;

        let backend_type = source_params
            .remove("type")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        let mut opt = BTreeMap::new();
        opt.insert("noObscure".to_string(), serde_json::Value::Bool(true));
        opt.insert("nonInteractive".to_string(), serde_json::Value::Bool(true));

        self.rc_client
            .config_create(new_name, &backend_type, source_params, opt)
            .await
            .map_err(|e| CoreError::Validation(format!("Failed to create duplicate: {}", e)))?;

        // Verify new remote exists
        let _ = self
            .rc_client
            .config_get(new_name)
            .await
            .map_err(|e| CoreError::Validation(format!("Verification of duplicate failed: {}", e)))?;

        Ok(())
    }

    /// Emulates rename remote via RC in safe sequence (CF-10)
    async fn rename_remote_emulation(
        &self,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), CoreError> {
        // 1. Get source params
        let mut source_params = self.rc_client.config_get(old_name).await.map_err(|e| {
            CoreError::NotFound(format!("Remote '{}' not found: {}", old_name, e))
        })?;

        let backend_type = source_params
            .remove("type")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        // 2. Create the copy first
        let mut opt = BTreeMap::new();
        opt.insert("noObscure".to_string(), serde_json::Value::Bool(true));
        opt.insert("nonInteractive".to_string(), serde_json::Value::Bool(true));

        self.rc_client
            .config_create(new_name, &backend_type, source_params, opt)
            .await
            .map_err(|e| CoreError::Validation(format!("Rename step (create new) failed: {}", e)))?;

        // 3. Update referencing remotes found in dump
        if let Ok(dump) = self.rc_client.config_dump().await {
            let mut remotes_list = Vec::new();
            for (sec_name, sec_val) in &dump {
                if let Some(map) = sec_val.as_object() {
                    let mut r = Remote::new(sec_name, "").unwrap_or_else(|_| Remote {
                        name: sec_name.clone(),
                        backend_type: String::new(),
                        parameters: BTreeMap::new(),
                        is_env_defined: false,
                        is_encrypted: false,
                    });
                    for (k, v) in map {
                        r.parameters.insert(k.clone(), v.clone());
                    }
                    remotes_list.push(r);
                }
            }

            let graph = RemoteDependencyGraph::build(&remotes_list);
            let referrers = graph.referrers_of(old_name);

            for ref_name in referrers {
                if let Ok(mut ref_params) = self.rc_client.config_get(&ref_name).await {
                    let mut changed = false;
                    let target_old = format!("{}:", old_name);
                    let target_new = format!("{}:", new_name);

                    if let Some(r_val) = ref_params.get_mut("remote") {
                        if let Some(s) = r_val.as_str() {
                            if s.starts_with(&target_old) {
                                *r_val = serde_json::Value::String(s.replace(&target_old, &target_new));
                                changed = true;
                            }
                        }
                    }

                    if let Some(up_val) = ref_params.get_mut("upstreams") {
                        if let Some(s) = up_val.as_str() {
                            if s.contains(&target_old) {
                                *up_val = serde_json::Value::String(s.replace(&target_old, &target_new));
                                changed = true;
                            }
                        }
                    }

                    if changed {
                        let _ = self
                            .rc_client
                            .config_update(&ref_name, ref_params, BTreeMap::new())
                            .await;
                    }
                }
            }
        }

        // 4. Verify new remote exists
        let _ = self
            .rc_client
            .config_get(new_name)
            .await
            .map_err(|e| CoreError::Validation(format!("Verification of renamed remote failed: {}", e)))?;

        // 5. Delete old remote
        let _ = self.rc_client.config_delete(old_name).await;

        Ok(())
    }

    async fn take_pre_mutation_snapshot_if_needed(&self) -> Result<(), CoreError> {
        let conf_path = match &self.config_file_path {
            Some(p) => p.clone(),
            None => match self.rc_client.config_paths().await {
                Ok(paths) => match paths.config {
                    Some(p) => Utf8PathBuf::from(p),
                    None => return Ok(()),
                },
                Err(_) => return Ok(()),
            },
        };

        if conf_path.exists() {
            if let Ok(content) = fs::read(&conf_path).await {
                let _ = self
                    .snapshot_store
                    .save_snapshot(&content, SnapshotReason::PreMutation)
                    .await;
            }
        }

        Ok(())
    }
}

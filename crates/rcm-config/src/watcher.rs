use std::sync::Arc;
use std::time::{Duration, Instant};
use camino::Utf8PathBuf;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::error;
use rcm_core::SnapshotReason;
use crate::snapshot::SnapshotStore;

pub struct ConfigWatcher {
    path: Utf8PathBuf,
    snapshot_store: Arc<SnapshotStore>,
    last_hash: Arc<Mutex<Option<String>>>,
    last_snapshot_time: Arc<Mutex<Option<Instant>>>,
    _watcher: Option<RecommendedWatcher>,
}

impl ConfigWatcher {
    pub fn new(path: Utf8PathBuf, snapshot_store: Arc<SnapshotStore>) -> Self {
        Self {
            path,
            snapshot_store,
            last_hash: Arc::new(Mutex::new(None)),
            last_snapshot_time: Arc::new(Mutex::new(None)),
            _watcher: None,
        }
    }

    pub async fn check_for_changes(&self) -> Option<String> {
        if !self.path.exists() {
            return None;
        }

        let content = tokio::fs::read(&self.path).await.ok()?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = format!("{:x}", hasher.finalize());

        let mut last_h = self.last_hash.lock().await;
        if let Some(prev) = last_h.as_ref() {
            if *prev == hash {
                return None; // No change
            }
        }

        *last_h = Some(hash.clone());

        // Rate-limit external-change snapshots (<= 1 per 10 min per BK-1 / CF-9)
        let mut last_snap = self.last_snapshot_time.lock().await;
        let now = Instant::now();
        let should_snapshot = match *last_snap {
            Some(t) => now.duration_since(t) >= Duration::from_secs(600),
            None => true,
        };

        if should_snapshot {
            *last_snap = Some(now);
            let _ = self
                .snapshot_store
                .save_snapshot(&content, SnapshotReason::ExternalChange)
                .await;
        }

        Some(hash)
    }

    pub fn start_watch<F>(&mut self, on_change: F) -> Result<(), notify::Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let path = self.path.clone();
        let last_hash = self.last_hash.clone();
        let last_snap = self.last_snapshot_time.clone();
        let store = self.snapshot_store.clone();
        let cb = Arc::new(on_change);

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    ) {
                        let p = path.clone();
                        let lh = last_hash.clone();
                        let ls = last_snap.clone();
                        let st = store.clone();
                        let cb_clone = cb.clone();

                        // Spawn async debounced check
                        tokio::spawn(async move {
                            sleep(Duration::from_millis(250)).await;
                            if let Ok(content) = tokio::fs::read(&p).await {
                                let mut hasher = Sha256::new();
                                hasher.update(&content);
                                let hash = format!("{:x}", hasher.finalize());

                                let mut cur_h = lh.lock().await;
                                if cur_h.as_ref() != Some(&hash) {
                                    *cur_h = Some(hash);

                                    let mut cur_snap = ls.lock().await;
                                    let now = Instant::now();
                                    let allowed = match *cur_snap {
                                        Some(t) => now.duration_since(t) >= Duration::from_secs(600),
                                        None => true,
                                    };
                                    if allowed {
                                        *cur_snap = Some(now);
                                        let _ = st
                                            .save_snapshot(&content, SnapshotReason::ExternalChange)
                                            .await;
                                    }
                                    cb_clone();
                                }
                            }
                        });
                    }
                }
                Err(e) => error!("Watcher error: {:?}", e),
            },
            notify::Config::default(),
        )?;

        let std_path = self.path.as_std_path();
        if let Some(parent) = std_path.parent() {
            let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
        }

        self._watcher = Some(watcher);
        Ok(())
    }
}

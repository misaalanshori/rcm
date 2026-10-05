use std::collections::HashSet;
use rcm_core::{MountProfile, MountTarget, ServeProfile};
use rcm_rc::types::{MountInfo, ServeInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcilerAction {
    StartMount(MountProfile),
    StartServe(ServeProfile),
    MarkUnmanagedMount(MountInfo),
    MarkUnmanagedServe(ServeInfo),
}

pub struct Reconciler;

impl Reconciler {
    /// Computes diff between desired state and actual running mounts/serves (§7.6)
    pub fn compute_actions(
        desired_mounts: &[MountProfile],
        desired_serves: &[ServeProfile],
        actual_mounts: &[MountInfo],
        actual_serves: &[ServeInfo],
    ) -> Vec<ReconcilerAction> {
        let mut actions = Vec::new();

        // 1. Mounts
        let mut matched_actual_mount_indices = HashSet::new();

        for profile in desired_mounts {
            if !profile.autostart {
                continue;
            }

            let profile_target = normalize_mount_target(&profile.target);
            let mut found = false;

            for (idx, actual) in actual_mounts.iter().enumerate() {
                let actual_point = normalize_str(&actual.mount_point);
                if profile_target == actual_point {
                    found = true;
                    matched_actual_mount_indices.insert(idx);
                    break;
                }
            }

            if !found {
                actions.push(ReconcilerAction::StartMount(profile.clone()));
            }
        }

        // Unmanaged mounts (MT-7)
        for (idx, actual) in actual_mounts.iter().enumerate() {
            if !matched_actual_mount_indices.contains(&idx) {
                actions.push(ReconcilerAction::MarkUnmanagedMount(actual.clone()));
            }
        }

        // 2. Serves
        let mut matched_actual_serve_indices = HashSet::new();

        for profile in desired_serves {
            if !profile.autostart {
                continue;
            }

            let profile_proto = profile.protocol.to_string().to_lowercase();
            let profile_fs = profile.remote.trim().to_lowercase();
            let mut found = false;

            for (idx, actual) in actual_serves.iter().enumerate() {
                if actual.protocol.to_lowercase() == profile_proto
                    && actual.fs.trim().to_lowercase() == profile_fs
                {
                    found = true;
                    matched_actual_serve_indices.insert(idx);
                    break;
                }
            }

            if !found {
                actions.push(ReconcilerAction::StartServe(profile.clone()));
            }
        }

        // Unmanaged serves
        for (idx, actual) in actual_serves.iter().enumerate() {
            if !matched_actual_serve_indices.contains(&idx) {
                actions.push(ReconcilerAction::MarkUnmanagedServe(actual.clone()));
            }
        }

        actions
    }
}

fn normalize_mount_target(target: &MountTarget) -> String {
    match target {
        MountTarget::DriveLetter(c) => format!("{}:", c.to_ascii_lowercase()),
        MountTarget::AutoDriveLetter => "*".to_string(),
        MountTarget::Folder(p) => normalize_str(p.as_str()),
        MountTarget::Unc(u) => normalize_str(u),
    }
}

fn normalize_str(s: &str) -> String {
    let clean = s.trim().replace('/', "\\").to_ascii_lowercase();
    clean.trim_end_matches('\\').to_string()
}

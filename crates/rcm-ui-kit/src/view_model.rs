use std::fmt;
use serde::{Deserialize, Serialize};
use rcm_core::{DaemonState, Id};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarDestination {
    Home,
    Remotes,
    Mounts,
    Serves,
    Files,
    Settings,
}

impl fmt::Display for SidebarDestination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SidebarDestination::Home => write!(f, "Home"),
            SidebarDestination::Remotes => write!(f, "Remotes"),
            SidebarDestination::Mounts => write!(f, "Mounts"),
            SidebarDestination::Serves => write!(f, "Serves"),
            SidebarDestination::Files => write!(f, "Files"),
            SidebarDestination::Settings => write!(f, "Settings"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardViewModel {
    pub daemon_state: DaemonState,
    pub active_mounts_count: usize,
    pub active_serves_count: usize,
    pub running_jobs_count: usize,
    pub recent_errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteItemViewModel {
    pub name: String,
    pub backend_type: String,
    pub referrers: Vec<String>,
    pub is_env_defined: bool,
    pub is_encrypted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountItemViewModel {
    pub id: Id,
    pub name: String,
    pub remote: String,
    pub target: String,
    pub preset: String,
    pub is_mounted: bool,
    pub cache_used_bytes: Option<u64>,
    pub upload_queue_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServeItemViewModel {
    pub id: Id,
    pub name: String,
    pub remote: String,
    pub protocol: String,
    pub addr: String,
    pub is_running: bool,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntryViewModel {
    pub name: String,
    pub path: String,
    pub size_bytes: i64,
    pub is_dir: bool,
    pub mod_time: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPaletteItem {
    pub title: String,
    pub subtitle: Option<String>,
    pub shortcut: Option<String>,
    pub destination: Option<SidebarDestination>,
    pub action_id: String,
}

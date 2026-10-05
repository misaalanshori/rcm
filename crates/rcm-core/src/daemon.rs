use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DaemonState {
    Stopped,
    Starting,
    Ready {
        execute_id: String,
        version: String,
        pid: u32,
        addr: String,
    },
    Degraded {
        reason: String,
    },
    Stopping,
    Crashed {
        exit_code: Option<i32>,
        last_error: String,
    },
    Failed {
        reason: String,
    },
}

impl DaemonState {
    pub fn is_ready(&self) -> bool {
        matches!(self, DaemonState::Ready { .. })
    }

    pub fn is_stopped(&self) -> bool {
        matches!(self, DaemonState::Stopped | DaemonState::Failed { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonAdoptInfo {
    pub pid: u32,
    pub start_time: u64,
    pub exe_path: Utf8PathBuf,
    pub config_path: Utf8PathBuf,
    pub addr: String,
    pub execute_id: String,
    pub auth_user: String,
    pub auth_pass: String,
}

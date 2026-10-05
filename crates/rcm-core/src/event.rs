use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use crate::daemon::DaemonState;
use crate::id::Id;
use crate::job::JobStats;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum RcmEvent {
    DaemonStateChanged(DaemonState),
    RcdRestarted {
        execute_id: String,
    },
    ConfigChanged {
        reason: String,
    },
    ExternalConfigDetected {
        path: Utf8PathBuf,
    },
    MountStateChanged {
        profile_id: Id,
        status: String,
        error: Option<String>,
    },
    ServeStateChanged {
        profile_id: Id,
        status: String,
        error: Option<String>,
    },
    JobProgress {
        profile_id: Id,
        job_id: i64,
        stats: JobStats,
    },
    JobFinished {
        profile_id: Id,
        job_id: i64,
        success: bool,
        error: Option<String>,
    },
}

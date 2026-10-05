use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use crate::error::CoreError;
use crate::id::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotReason {
    PreMutation,
    Scheduled,
    Manual,
    PreRestore,
    ExternalChange,
    FirstRun,
}

impl fmt::Display for SnapshotReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnapshotReason::PreMutation => write!(f, "pre-mutation"),
            SnapshotReason::Scheduled => write!(f, "scheduled"),
            SnapshotReason::Manual => write!(f, "manual"),
            SnapshotReason::PreRestore => write!(f, "pre-restore"),
            SnapshotReason::ExternalChange => write!(f, "external-change"),
            SnapshotReason::FirstRun => write!(f, "first-run"),
        }
    }
}

impl FromStr for SnapshotReason {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pre-mutation" => Ok(SnapshotReason::PreMutation),
            "scheduled" => Ok(SnapshotReason::Scheduled),
            "manual" => Ok(SnapshotReason::Manual),
            "pre-restore" => Ok(SnapshotReason::PreRestore),
            "external-change" => Ok(SnapshotReason::ExternalChange),
            "first-run" => Ok(SnapshotReason::FirstRun),
            _ => Err(CoreError::Validation(format!("Unknown snapshot reason '{}'", s))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub id: Id,
    pub filename: String,
    pub timestamp_utc: u64,
    pub reason: SnapshotReason,
    pub content_hash: String,
    pub file_size_bytes: u64,
    pub remote_names: Vec<String>,
    pub is_encrypted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MutationIntent {
    CreateRemote {
        name: String,
        backend_type: String,
        parameters: BTreeMap<String, serde_json::Value>,
        opt: BTreeMap<String, String>,
    },
    UpdateRemote {
        name: String,
        parameters: BTreeMap<String, serde_json::Value>,
        opt: BTreeMap<String, String>,
    },
    DeleteRemote {
        name: String,
    },
    RenameRemote {
        old_name: String,
        new_name: String,
    },
    DuplicateRemote {
        source_name: String,
        new_name: String,
    },
    UnlockConfig {
        password_keyring_key: String,
    },
    SetConfigPath {
        path: Utf8PathBuf,
    },
}

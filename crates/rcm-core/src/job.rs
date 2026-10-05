use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::error::CoreError;
use crate::id::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobOperation {
    Sync,
    Copy,
    Move,
    Bisync,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobFilter {
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub min_size: Option<String>,
    #[serde(default)]
    pub max_size: Option<String>,
    #[serde(default)]
    pub min_age: Option<String>,
    #[serde(default)]
    pub max_age: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobProfile {
    pub id: Id,
    pub name: String,
    pub operation: JobOperation,
    pub src_fs: String,
    pub dst_fs: String,
    #[serde(default)]
    pub filter: JobFilter,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub max_delete: Option<i64>,
    #[serde(default)]
    pub flags: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub schedule: Option<String>, // Cron or interval expression
}

impl JobProfile {
    pub fn new(
        name: impl Into<String>,
        operation: JobOperation,
        src_fs: impl Into<String>,
        dst_fs: impl Into<String>,
    ) -> Self {
        Self {
            id: Id::new(),
            name: name.into(),
            operation,
            src_fs: src_fs.into(),
            dst_fs: dst_fs.into(),
            filter: JobFilter::default(),
            dry_run: false,
            max_delete: None,
            flags: BTreeMap::new(),
            schedule: None,
        }
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::Validation("Job name cannot be empty".to_string()));
        }
        if self.src_fs.trim().is_empty() {
            return Err(CoreError::Validation("Source filesystem cannot be empty".to_string()));
        }
        if self.dst_fs.trim().is_empty() {
            return Err(CoreError::Validation("Destination filesystem cannot be empty".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct JobStats {
    pub bytes: i64,
    pub total_bytes: i64,
    pub speed: f64,
    pub speed_average: f64,
    pub transfers: i64,
    pub total_transfers: i64,
    pub checks: i64,
    pub total_checks: i64,
    pub deletes: i64,
    pub errors: i64,
    pub fatal_error: bool,
    pub eta: Option<i64>,
}

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::error::CoreError;
use crate::id::Id;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServeProtocol {
    Http,
    Webdav,
    Ftp,
    Sftp,
    Nfs,
    Dlna,
    S3,
    Restic,
    Docker,
    #[serde(untagged)]
    Custom(String),
}

impl fmt::Display for ServeProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServeProtocol::Http => write!(f, "http"),
            ServeProtocol::Webdav => write!(f, "webdav"),
            ServeProtocol::Ftp => write!(f, "ftp"),
            ServeProtocol::Sftp => write!(f, "sftp"),
            ServeProtocol::Nfs => write!(f, "nfs"),
            ServeProtocol::Dlna => write!(f, "dlna"),
            ServeProtocol::S3 => write!(f, "s3"),
            ServeProtocol::Restic => write!(f, "restic"),
            ServeProtocol::Docker => write!(f, "docker"),
            ServeProtocol::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl FromStr for ServeProtocol {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "http" => ServeProtocol::Http,
            "webdav" => ServeProtocol::Webdav,
            "ftp" => ServeProtocol::Ftp,
            "sftp" => ServeProtocol::Sftp,
            "nfs" => ServeProtocol::Nfs,
            "dlna" => ServeProtocol::Dlna,
            "s3" => ServeProtocol::S3,
            "restic" => ServeProtocol::Restic,
            "docker" => ServeProtocol::Docker,
            _ => ServeProtocol::Custom(s.to_string()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServeProfile {
    pub id: Id,
    pub name: String,
    pub remote: String,
    pub protocol: ServeProtocol,
    pub addr: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub pass: Option<String>,
    #[serde(default)]
    pub vfs_options: BTreeMap<String, String>,
    #[serde(default)]
    pub extra_flags: Vec<String>,
}

impl ServeProfile {
    pub fn new(
        name: impl Into<String>,
        remote: impl Into<String>,
        protocol: ServeProtocol,
        addr: impl Into<String>,
    ) -> Self {
        Self {
            id: Id::new(),
            name: name.into(),
            remote: remote.into(),
            protocol,
            addr: addr.into(),
            read_only: false,
            autostart: false,
            user: None,
            pass: None,
            vfs_options: BTreeMap::new(),
            extra_flags: Vec::new(),
        }
    }

    pub fn is_loopback(&self) -> bool {
        let addr = self.addr.trim();
        addr.starts_with("127.0.0.1")
            || addr.starts_with("localhost")
            || addr.starts_with("[::1]")
            || addr.starts_with("127.")
    }

    /// Validates profile rules per SV-3 (safe defaults: loopback default; non-loopback bind forces auth)
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::Validation("Serve name cannot be empty".to_string()));
        }
        if self.remote.trim().is_empty() {
            return Err(CoreError::Validation("Serve remote cannot be empty".to_string()));
        }
        if self.addr.trim().is_empty() {
            return Err(CoreError::Validation("Serve address cannot be empty".to_string()));
        }
        if !self.is_loopback() && (self.user.is_none() || self.pass.is_none()) {
            return Err(CoreError::Validation(
                "Non-loopback bind address requires authentication (user and pass)".to_string(),
            ));
        }
        Ok(())
    }
}

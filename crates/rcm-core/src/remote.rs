use std::collections::{BTreeMap, HashMap, HashSet};
use serde::{Deserialize, Serialize};
use crate::error::CoreError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remote {
    pub name: String,
    #[serde(rename = "type")]
    pub backend_type: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub is_env_defined: bool,
    #[serde(default)]
    pub is_encrypted: bool,
}

impl Remote {
    pub fn new(name: impl Into<String>, backend_type: impl Into<String>) -> Result<Self, CoreError> {
        let name = name.into();
        Self::validate_name(&name)?;
        Ok(Self {
            name,
            backend_type: backend_type.into(),
            parameters: BTreeMap::new(),
            is_env_defined: false,
            is_encrypted: false,
        })
    }

    pub fn validate_name(name: &str) -> Result<(), CoreError> {
        if name.trim().is_empty() {
            return Err(CoreError::InvalidRemoteName("Remote name cannot be empty".to_string()));
        }
        if name.contains(':') || name.contains('\\') || name.contains('/') {
            return Err(CoreError::InvalidRemoteName(format!(
                "Remote name '{}' contains forbidden characters (':', '\\', '/')",
                name
            )));
        }
        if name.len() == 1 && name.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
            // Single-letter remote names can collide with Windows drive letters
            // It's allowed but warning-flagged; we don't hard reject, but validation notes can check it.
        }
        Ok(())
    }

    pub fn upstream_references(&self) -> Vec<String> {
        let mut refs = Vec::new();
        // Common rclone keys that reference other remotes:
        // crypt, alias, chunker, compress, hasher, cache -> "remote"
        // union, combine -> "upstreams"
        if let Some(r) = self.parameters.get("remote").and_then(|v| v.as_str()) {
            if let Some(colon_pos) = r.find(':') {
                refs.push(r[..colon_pos].to_string());
            } else if !r.is_empty() {
                refs.push(r.to_string());
            }
        }
        if let Some(upstreams) = self.parameters.get("upstreams").and_then(|v| v.as_str()) {
            for entry in upstreams.split_whitespace() {
                if let Some(colon_pos) = entry.find(':') {
                    refs.push(entry[..colon_pos].to_string());
                } else if !entry.is_empty() {
                    refs.push(entry.to_string());
                }
            }
        }
        refs
    }
}

/// Dependency graph tracking relations between remotes (CF-6)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RemoteDependencyGraph {
    /// Remote -> set of remotes it points to (direct upstreams)
    pub outgoing: HashMap<String, HashSet<String>>,
    /// Remote -> set of remotes that point to it (referrers)
    pub incoming: HashMap<String, HashSet<String>>,
}

impl RemoteDependencyGraph {
    pub fn build<'a>(remotes: impl IntoIterator<Item = &'a Remote>) -> Self {
        let mut graph = Self::default();
        for r in remotes {
            let upstreams = r.upstream_references();
            for up in upstreams {
                graph.outgoing.entry(r.name.clone()).or_default().insert(up.clone());
                graph.incoming.entry(up).or_default().insert(r.name.clone());
            }
        }
        graph
    }

    /// Returns list of remotes depending on the given remote (CF-6 delete check)
    pub fn referrers_of(&self, remote_name: &str) -> HashSet<String> {
        self.incoming.get(remote_name).cloned().unwrap_or_default()
    }
}

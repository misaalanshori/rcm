use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OptionExample {
    #[serde(rename = "Value", default)]
    pub value: String,
    #[serde(rename = "Help", default)]
    pub help: String,
    #[serde(rename = "Provider", default)]
    pub provider: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OptionSchema {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Help", default)]
    pub help: String,
    #[serde(rename = "Default", default)]
    pub default: Option<serde_json::Value>,
    #[serde(rename = "Examples", default)]
    pub examples: Vec<OptionExample>,
    #[serde(rename = "Required", default)]
    pub required: bool,
    #[serde(rename = "IsPassword", default)]
    pub is_password: bool,
    #[serde(rename = "Sensitive", default)]
    pub sensitive: bool,
    #[serde(rename = "Advanced", default)]
    pub advanced: bool,
    #[serde(rename = "Exclusive", default)]
    pub exclusive: bool,
    #[serde(rename = "Type", default)]
    pub option_type: String,
    #[serde(rename = "Groups", default)]
    pub groups: String,
    #[serde(rename = "Provider", default)]
    pub provider: String,
    #[serde(rename = "Hide", default)]
    pub hide: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProviderInfo {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Description", default)]
    pub description: String,
    #[serde(rename = "Prefix", default)]
    pub prefix: String,
    #[serde(rename = "Options", default)]
    pub options: Vec<OptionSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProvidersResponse {
    #[serde(default)]
    pub providers: Vec<ProviderInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VersionResponse {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub decomposed: Option<Vec<i64>>,
    #[serde(rename = "isBeta", default)]
    pub is_beta: bool,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub arch: String,
    #[serde(rename = "goVersion", default)]
    pub go_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PidResponse {
    #[serde(default)]
    pub pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListRemotesResponse {
    #[serde(default)]
    pub remotes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PathsResponse {
    #[serde(default)]
    pub config: Option<String>,
    #[serde(default)]
    pub cache: Option<String>,
    #[serde(default)]
    pub temp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MountInfo {
    #[serde(rename = "Fs", default)]
    pub fs: String,
    #[serde(rename = "MountPoint", default)]
    pub mount_point: String,
    #[serde(rename = "MountedOn", default)]
    pub mounted_on: Option<String>,
    #[serde(rename = "Type", default)]
    pub mount_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ListMountsResponse {
    #[serde(rename = "mountPoints", default)]
    pub mount_points: Vec<MountInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MountTypesResponse {
    #[serde(rename = "mountTypes", default)]
    pub mount_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ServeInfo {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(rename = "type", default)]
    pub protocol: String,
    #[serde(default)]
    pub fs: String,
    #[serde(default)]
    pub addr: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ListServesResponse {
    #[serde(default)]
    pub serves: Vec<ServeInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ServeTypesResponse {
    #[serde(default)]
    pub types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct JobStatusResponse {
    #[serde(default)]
    pub finished: bool,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
    #[serde(rename = "startTime", default)]
    pub start_time: Option<String>,
    #[serde(rename = "endTime", default)]
    pub end_time: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ListEntry {
    #[serde(rename = "Path", default)]
    pub path: String,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Size", default)]
    pub size: i64,
    #[serde(rename = "MimeType", default)]
    pub mime_type: String,
    #[serde(rename = "ModTime", default)]
    pub mod_time: String,
    #[serde(rename = "IsDir", default)]
    pub is_dir: bool,
    #[serde(rename = "Tier", default)]
    pub tier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ListResponse {
    #[serde(default)]
    pub list: Vec<ListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AboutResponse {
    #[serde(default)]
    pub total: Option<i64>,
    #[serde(default)]
    pub used: Option<i64>,
    #[serde(default)]
    pub trashed: Option<i64>,
    #[serde(default)]
    pub other: Option<i64>,
    #[serde(default)]
    pub free: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PublicLinkResponse {
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WizardStepResponse {
    #[serde(rename = "State", default)]
    pub state: String,
    #[serde(rename = "Option", default)]
    pub option: Option<OptionSchema>,
    #[serde(rename = "Error", default)]
    pub error: String,
    #[serde(rename = "Result", default)]
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OAuthStatusResponse {
    #[serde(rename = "AuthUrl", default)]
    pub auth_url: Option<String>,
    #[serde(rename = "Status", default)]
    pub status: String,
    #[serde(rename = "Error", default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RcCallInfo {
    #[serde(rename = "Path", default)]
    pub path: String,
    #[serde(rename = "Title", default)]
    pub title: String,
    #[serde(rename = "Help", default)]
    pub help: String,
    #[serde(rename = "NoAuth", default)]
    pub no_auth: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RcListResponse {
    #[serde(default)]
    pub commands: Option<Vec<RcCallInfo>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OptionsInfoResponse {
    #[serde(flatten)]
    pub groups: BTreeMap<String, Vec<OptionSchema>>,
}

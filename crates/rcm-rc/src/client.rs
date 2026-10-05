use std::collections::BTreeMap;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::error::{RcError, RcRawError};
use crate::types::*;

#[derive(Clone)]
pub struct RcClient {
    base_url: String,
    auth_header: Option<String>,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>>,
}

impl Drop for RcClient {
    fn drop(&mut self) {
        if let Some(mut header) = self.auth_header.take() {
            header.zeroize();
        }
    }
}

impl RcClient {
    pub fn new(base_url: impl Into<String>, user: Option<&str>, pass: Option<&str>) -> Self {
        let auth_header = match (user, pass) {
            (Some(u), Some(p)) => {
                let raw = format!("{}:{}", u, p);
                let encoded = simple_base64_encode(raw.as_bytes());
                Some(format!("Basic {}", encoded))
            }
            _ => None,
        };

        let client = Client::builder(TokioExecutor::new()).build_http();

        let mut base_url = base_url.into();
        if base_url.ends_with('/') {
            base_url.pop();
        }

        Self {
            base_url,
            auth_header,
            client,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Generic call helper (R1, §7.2)
    pub async fn call<Req: Serialize, Resp: DeserializeOwned>(
        &self,
        path: &str,
        params: Req,
    ) -> Result<Resp, RcError> {
        let path = path.trim_start_matches('/');
        let url = format!("{}/{}", self.base_url, path);

        let json_bytes = serde_json::to_vec(&params)?;
        let mut req_builder = Request::post(&url)
            .header("Content-Type", "application/json");

        if let Some(ref auth) = self.auth_header {
            req_builder = req_builder.header("Authorization", auth);
        }

        let request = req_builder
            .body(Full::new(Bytes::from(json_bytes)))
            .map_err(|e| RcError::Connection(format!("Failed to build HTTP request: {}", e)))?;

        let response = self
            .client
            .request(request)
            .await
            .map_err(|e| RcError::Connection(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let body_bytes = response
            .into_body()
            .collect()
            .await
            .map_err(|e| RcError::Connection(format!("Failed to read HTTP body: {}", e)))?
            .to_bytes();

        if !status.is_success() {
            let raw_err: Option<RcRawError> = serde_json::from_slice(&body_bytes).ok();
            let msg = raw_err
                .as_ref()
                .map(|r| r.error.clone())
                .unwrap_or_else(|| String::from_utf8_lossy(&body_bytes).to_string());

            if let Some(raw) = raw_err {
                return Err(RcError::Api {
                    status: Some(status.as_u16()),
                    message: msg,
                    path: Some(path.to_string()),
                    raw,
                });
            } else {
                return Err(RcError::Http {
                    status: status.as_u16(),
                    message: msg,
                    raw: None,
                });
            }
        }

        let resp: Resp = serde_json::from_slice(&body_bytes)?;
        Ok(resp)
    }

    // --- Core RC endpoints ---

    pub async fn noop(&self) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("rc/noop", serde_json::json!({})).await?;
        Ok(())
    }

    pub async fn list_commands(&self) -> Result<Vec<String>, RcError> {
        let res: RcListResponse = self.call("rc/list", serde_json::json!({})).await?;
        Ok(res.commands.unwrap_or_default())
    }

    pub async fn version(&self) -> Result<VersionResponse, RcError> {
        self.call("core/version", serde_json::json!({})).await
    }

    pub async fn pid(&self) -> Result<u32, RcError> {
        let res: PidResponse = self.call("core/pid", serde_json::json!({})).await?;
        Ok(res.pid)
    }

    pub async fn quit(&self) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("core/quit", serde_json::json!({})).await?;
        Ok(())
    }

    pub async fn obscure(&self, clear: &str) -> Result<String, RcError> {
        #[derive(Deserialize)]
        struct ObscureResp {
            obscured: String,
        }
        let res: ObscureResp = self.call("core/obscure", serde_json::json!({ "clear": clear })).await?;
        Ok(res.obscured)
    }

    pub async fn core_stats(&self, group: Option<&str>) -> Result<serde_json::Value, RcError> {
        let mut body = serde_json::Map::new();
        if let Some(g) = group {
            body.insert("group".to_string(), serde_json::Value::String(g.to_string()));
        }
        self.call("core/stats", body).await
    }

    pub async fn core_bwlimit(&self, rate: &str) -> Result<serde_json::Value, RcError> {
        self.call("core/bwlimit", serde_json::json!({ "rate": rate })).await
    }

    pub async fn core_command(&self, command: &str, args: Vec<String>, opt: BTreeMap<String, serde_json::Value>) -> Result<serde_json::Value, RcError> {
        self.call("core/command", serde_json::json!({
            "command": command,
            "arg": args,
            "opt": opt,
        })).await
    }

    // --- Config endpoints ---

    pub async fn config_list_remotes(&self) -> Result<Vec<String>, RcError> {
        let res: ListRemotesResponse = self.call("config/listremotes", serde_json::json!({})).await?;
        Ok(res.remotes.unwrap_or_default())
    }

    pub async fn config_get(&self, name: &str) -> Result<BTreeMap<String, serde_json::Value>, RcError> {
        self.call("config/get", serde_json::json!({ "name": name })).await
    }

    pub async fn config_dump(&self) -> Result<BTreeMap<String, serde_json::Value>, RcError> {
        self.call("config/dump", serde_json::json!({})).await
    }

    pub async fn config_providers(&self) -> Result<Vec<ProviderInfo>, RcError> {
        let res: ProvidersResponse = self.call("config/providers", serde_json::json!({})).await?;
        Ok(res.providers)
    }

    pub async fn config_create(
        &self,
        name: &str,
        backend_type: &str,
        parameters: BTreeMap<String, serde_json::Value>,
        opt: BTreeMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, RcError> {
        self.call("config/create", serde_json::json!({
            "name": name,
            "type": backend_type,
            "parameters": parameters,
            "opt": opt,
        })).await
    }

    pub async fn config_update(
        &self,
        name: &str,
        parameters: BTreeMap<String, serde_json::Value>,
        opt: BTreeMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, RcError> {
        self.call("config/update", serde_json::json!({
            "name": name,
            "parameters": parameters,
            "opt": opt,
        })).await
    }

    pub async fn config_delete(&self, name: &str) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("config/delete", serde_json::json!({ "name": name })).await?;
        Ok(())
    }

    pub async fn config_paths(&self) -> Result<PathsResponse, RcError> {
        self.call("config/paths", serde_json::json!({})).await
    }

    pub async fn config_set_path(&self, path: &str) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("config/setpath", serde_json::json!({ "path": path })).await?;
        Ok(())
    }

    pub async fn config_unlock(&self, pass: &str) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("config/unlock", serde_json::json!({ "pass": pass })).await?;
        Ok(())
    }

    pub async fn config_oauth_status(&self) -> Result<OAuthStatusResponse, RcError> {
        self.call("config/oauthstatus", serde_json::json!({})).await
    }

    pub async fn config_oauth_stop(&self) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("config/oauthstop", serde_json::json!({})).await?;
        Ok(())
    }

    pub async fn fscache_clear(&self) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("fscache/clear", serde_json::json!({})).await?;
        Ok(())
    }

    // --- Mount endpoints ---

    pub async fn mount_mount(
        &self,
        fs: &str,
        mount_point: &str,
        mount_type: Option<&str>,
        options: BTreeMap<String, String>,
    ) -> Result<serde_json::Value, RcError> {
        let mut body = serde_json::json!({
            "fs": fs,
            "mountPoint": mount_point,
        });
        if let Some(mt) = mount_type {
            body["mountType"] = serde_json::Value::String(mt.to_string());
        }
        if let Some(obj) = body.as_object_mut() {
            for (k, v) in options {
                obj.insert(k, serde_json::Value::String(v));
            }
        }
        self.call("mount/mount", body).await
    }

    pub async fn mount_unmount(&self, mount_point: &str) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("mount/unmount", serde_json::json!({ "mountPoint": mount_point })).await?;
        Ok(())
    }

    pub async fn mount_unmount_all(&self) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("mount/unmountall", serde_json::json!({})).await?;
        Ok(())
    }

    pub async fn mount_list_mounts(&self) -> Result<Vec<MountInfo>, RcError> {
        let res: ListMountsResponse = self.call("mount/listmounts", serde_json::json!({})).await?;
        Ok(res.mount_points)
    }

    pub async fn mount_types(&self) -> Result<Vec<String>, RcError> {
        let res: MountTypesResponse = self.call("mount/types", serde_json::json!({})).await?;
        Ok(res.mount_types)
    }

    // --- Serve endpoints ---

    pub async fn serve_start(
        &self,
        protocol: &str,
        fs: &str,
        addr: &str,
        user: Option<&str>,
        pass: Option<&str>,
        vfs_options: BTreeMap<String, String>,
    ) -> Result<serde_json::Value, RcError> {
        let mut body = serde_json::json!({
            "type": protocol,
            "fs": fs,
            "addr": addr,
        });
        if let Some(u) = user {
            body["user"] = serde_json::Value::String(u.to_string());
        }
        if let Some(p) = pass {
            body["pass"] = serde_json::Value::String(p.to_string());
        }
        if let Some(obj) = body.as_object_mut() {
            for (k, v) in vfs_options {
                obj.insert(k, serde_json::Value::String(v));
            }
        }
        self.call("serve/start", body).await
    }

    pub async fn serve_stop(&self, id: u64) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("serve/stop", serde_json::json!({ "id": id })).await?;
        Ok(())
    }

    pub async fn serve_stop_all(&self) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("serve/stopall", serde_json::json!({})).await?;
        Ok(())
    }

    pub async fn serve_list(&self) -> Result<Vec<ServeInfo>, RcError> {
        let res: ListServesResponse = self.call("serve/list", serde_json::json!({})).await?;
        Ok(res.serves)
    }

    pub async fn serve_types(&self) -> Result<Vec<String>, RcError> {
        let res: ServeTypesResponse = self.call("serve/types", serde_json::json!({})).await?;
        Ok(res.types)
    }

    // --- Job endpoints ---

    pub async fn job_status(&self, job_id: i64) -> Result<JobStatusResponse, RcError> {
        self.call("job/status", serde_json::json!({ "jobid": job_id })).await
    }

    pub async fn job_stop(&self, job_id: i64) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("job/stop", serde_json::json!({ "jobid": job_id })).await?;
        Ok(())
    }

    pub async fn job_stop_all(&self) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("job/stopall", serde_json::json!({})).await?;
        Ok(())
    }

    pub async fn job_list(&self) -> Result<Vec<i64>, RcError> {
        #[derive(Deserialize)]
        struct JobListResp {
            #[serde(default)]
            jobids: Option<Vec<i64>>,
        }
        let res: JobListResp = self.call("job/list", serde_json::json!({})).await?;
        Ok(res.jobids.unwrap_or_default())
    }

    // --- Operations endpoints ---

    pub async fn operations_list(&self, fs: &str, remote: &str) -> Result<ListResponse, RcError> {
        self.call("operations/list", serde_json::json!({ "fs": fs, "remote": remote })).await
    }

    pub async fn operations_mkdir(&self, fs: &str, remote: &str) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("operations/mkdir", serde_json::json!({ "fs": fs, "remote": remote })).await?;
        Ok(())
    }

    pub async fn operations_purge(&self, fs: &str, remote: &str) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("operations/purge", serde_json::json!({ "fs": fs, "remote": remote })).await?;
        Ok(())
    }

    pub async fn operations_delete(&self, fs: &str, remote: &str) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("operations/deletefile", serde_json::json!({ "fs": fs, "remote": remote })).await?;
        Ok(())
    }

    pub async fn operations_about(&self, fs: &str) -> Result<AboutResponse, RcError> {
        self.call("operations/about", serde_json::json!({ "fs": fs })).await
    }

    pub async fn operations_public_link(&self, fs: &str, remote: &str) -> Result<PublicLinkResponse, RcError> {
        self.call("operations/publiclink", serde_json::json!({ "fs": fs, "remote": remote })).await
    }

    // --- Options endpoints ---

    pub async fn options_info(&self) -> Result<OptionsInfoResponse, RcError> {
        self.call("options/info", serde_json::json!({})).await
    }

    pub async fn options_get(&self) -> Result<serde_json::Value, RcError> {
        self.call("options/get", serde_json::json!({})).await
    }

    pub async fn options_set(&self, options: BTreeMap<String, serde_json::Value>) -> Result<(), RcError> {
        let _: serde_json::Value = self.call("options/set", options).await?;
        Ok(())
    }
}

/// Simple RFC 4648 standard base64 encoding without external dependencies (§7.15)
fn simple_base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);

    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        let i0 = (b0 >> 2) as usize;
        let i1 = (((b0 & 0x03) << 4) | (b1 >> 4)) as usize;
        let i2 = (((b1 & 0x0F) << 2) | (b2 >> 6)) as usize;
        let i3 = (b2 & 0x3F) as usize;

        out.push(TABLE[i0] as char);
        out.push(TABLE[i1] as char);

        if chunk.len() > 1 {
            out.push(TABLE[i2] as char);
        } else {
            out.push('=');
        }

        if chunk.len() > 2 {
            out.push(TABLE[i3] as char);
        } else {
            out.push('=');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(simple_base64_encode(b""), "");
        assert_eq!(simple_base64_encode(b"f"), "Zg==");
        assert_eq!(simple_base64_encode(b"fo"), "Zm8=");
        assert_eq!(simple_base64_encode(b"foo"), "Zm9v");
        assert_eq!(simple_base64_encode(b"admin:secret123"), "YWRtaW46c2VjcmV0MTIz");
    }
}

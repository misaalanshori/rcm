use std::future::Future;
use interprocess::local_socket::{
    tokio::{prelude::*, Listener, Stream},
    GenericNamespaced, ListenerOptions, ToNsName,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};
use rcm_core::CoreError;
use crate::protocol::{IpcError, IpcRequest, IpcResponse};

pub struct IpcServer {
    listener: Listener,
}

impl IpcServer {
    pub async fn bind(name_str: &str) -> Result<Self, CoreError> {
        let name = name_str.to_ns_name::<GenericNamespaced>().map_err(|e| {
            CoreError::Validation(format!("Invalid socket name '{}': {}", name_str, e))
        })?;

        let listener = ListenerOptions::new()
            .name(name)
            .create_tokio()
            .map_err(|e| CoreError::Validation(format!("Failed to bind local socket: {}", e)))?;

        info!("IPC Server bound to {}", name_str);
        Ok(Self { listener })
    }

    pub async fn run<H, Fut>(&self, handler: H)
    where
        H: Fn(IpcRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<serde_json::Value, IpcError>> + Send + 'static,
    {
        let handler = std::sync::Arc::new(handler);
        loop {
            match self.listener.accept().await {
                Ok(conn) => {
                    let h = handler.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(conn, h).await {
                            error!("IPC connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("IPC accept error: {}", e);
                    break;
                }
            }
        }
    }
}

async fn handle_connection<H, Fut>(conn: Stream, handler: std::sync::Arc<H>) -> Result<(), CoreError>
where
    H: Fn(IpcRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<serde_json::Value, IpcError>> + Send + 'static,
{
    let (reader, mut writer) = conn.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await.map_err(|e| {
            CoreError::Validation(format!("Failed to read line from IPC stream: {}", e))
        })?;

        if bytes_read == 0 {
            break; // Connection closed
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: IpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = IpcResponse {
                    id: 0,
                    result: None,
                    error: Some(IpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                };
                let mut out = serde_json::to_vec(&err_resp).unwrap_or_default();
                out.push(b'\n');
                let _ = writer.write_all(&out).await;
                continue;
            }
        };

        let req_id = req.id;
        let response = match handler(req).await {
            Ok(val) => IpcResponse {
                id: req_id,
                result: Some(val),
                error: None,
            },
            Err(err) => IpcResponse {
                id: req_id,
                result: None,
                error: Some(err),
            },
        };

        let mut out = serde_json::to_vec(&response).unwrap_or_default();
        out.push(b'\n');
        if let Err(e) = writer.write_all(&out).await {
            return Err(CoreError::Validation(format!("Failed to write IPC response: {}", e)));
        }
    }

    Ok(())
}

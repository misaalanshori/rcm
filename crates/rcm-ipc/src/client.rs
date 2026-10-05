use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use interprocess::local_socket::{
    tokio::{prelude::*, RecvHalf, SendHalf, Stream},
    GenericNamespaced, ToNsName,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use rcm_core::CoreError;
use crate::protocol::{IpcRequest, IpcResponse};

pub struct IpcClient {
    writer: Arc<Mutex<SendHalf>>,
    reader: Arc<Mutex<BufReader<RecvHalf>>>,
    next_id: AtomicU64,
}

impl IpcClient {
    pub async fn connect(name_str: &str) -> Result<Self, CoreError> {
        let name = name_str.to_ns_name::<GenericNamespaced>().map_err(|e| {
            CoreError::Validation(format!("Invalid socket name '{}': {}", name_str, e))
        })?;

        let stream = Stream::connect(name).await.map_err(|e| {
            CoreError::Connection(format!("Failed to connect to IPC socket '{}': {}", name_str, e))
        })?;

        let (reader, writer) = stream.split();

        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            reader: Arc::new(Mutex::new(BufReader::new(reader))),
            next_id: AtomicU64::new(1),
        })
    }

    pub async fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, CoreError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = IpcRequest {
            id,
            method: method.to_string(),
            params,
        };

        let mut data = serde_json::to_vec(&req).map_err(|e| {
            CoreError::Serialization(format!("Failed to serialize IPC request: {}", e))
        })?;
        data.push(b'\n');

        // Write request
        {
            let mut w = self.writer.lock().await;
            w.write_all(&data).await.map_err(|e| {
                CoreError::Connection(format!("Failed to write to IPC socket: {}", e))
            })?;
            w.flush().await.map_err(|e| {
                CoreError::Connection(format!("Failed to flush IPC socket: {}", e))
            })?;
        }

        // Read response
        let mut line = String::new();
        {
            let mut r = self.reader.lock().await;
            r.read_line(&mut line).await.map_err(|e| {
                CoreError::Connection(format!("Failed to read response line from IPC socket: {}", e))
            })?;
        }

        if line.is_empty() {
            return Err(CoreError::Connection("IPC connection closed by server".to_string()));
        }

        let resp: IpcResponse = serde_json::from_str(line.trim()).map_err(|e| {
            CoreError::Serialization(format!("Invalid IPC response JSON: {}. Line: {}", e, line))
        })?;

        if let Some(err) = resp.error {
            return Err(CoreError::Validation(format!(
                "IPC error ({}): {}",
                err.code, err.message
            )));
        }

        Ok(resp.result.unwrap_or(serde_json::Value::Null))
    }
}

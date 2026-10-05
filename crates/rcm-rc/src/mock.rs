use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub struct MockRcServer {
    addr: SocketAddr,
    routes: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    wizard_queue: Arc<Mutex<Vec<serde_json::Value>>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl MockRcServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let routes = Arc::new(Mutex::new(HashMap::new()));
        let wizard_queue = Arc::new(Mutex::new(Vec::new()));
        let (shutdown_tx, _) = broadcast::channel(1);

        let s_routes = routes.clone();
        let s_wizard = wizard_queue.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_res = listener.accept() => {
                        if let Ok((mut socket, _)) = accept_res {
                            let routes = s_routes.clone();
                            let wizard = s_wizard.clone();
                            tokio::spawn(async move {
                                let mut buf = vec![0u8; 8192];
                                let n = match socket.read(&mut buf).await {
                                    Ok(n) if n > 0 => n,
                                    _ => return,
                                };
                                let req_str = String::from_utf8_lossy(&buf[..n]);

                                // Simple HTTP request line parsing
                                let first_line = req_str.lines().next().unwrap_or("");
                                let path = first_line.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/');

                                let response_body = if path == "config/create" || path == "config/update" {
                                    let mut wq = wizard.lock().await;
                                    if !wq.is_empty() {
                                        wq.remove(0)
                                    } else {
                                        let r = routes.lock().await;
                                        r.get(path).cloned().unwrap_or(serde_json::json!({}))
                                    }
                                } else {
                                    let r = routes.lock().await;
                                    r.get(path).cloned().unwrap_or(serde_json::json!({}))
                                };

                                let body_bytes = serde_json::to_vec(&response_body).unwrap();
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    body_bytes.len()
                                );

                                let _ = socket.write_all(resp.as_bytes()).await;
                                let _ = socket.write_all(&body_bytes).await;
                                let _ = socket.flush().await;
                            });
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            routes,
            wizard_queue,
            shutdown_tx,
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }

    pub async fn set_route(&self, path: &str, response: serde_json::Value) {
        let mut r = self.routes.lock().await;
        r.insert(path.trim_start_matches('/').to_string(), response);
    }

    pub async fn enqueue_wizard_step(&self, step: serde_json::Value) {
        let mut wq = self.wizard_queue.lock().await;
        wq.push(step);
    }

    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

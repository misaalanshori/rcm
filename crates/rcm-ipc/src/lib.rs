pub mod client;
pub mod protocol;
pub mod server;

pub use client::IpcClient;
pub use protocol::{Handshake, IpcError, IpcNotification, IpcRequest, IpcResponse, RcConnectionInfo};
pub use server::IpcServer;

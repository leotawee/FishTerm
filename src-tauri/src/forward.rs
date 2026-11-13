//! Local port forwarding (`ssh -L`). One TcpListener per rule, bound to
//! 127.0.0.1; each accepted connection opens a `direct-tcpip` channel on the
//! session's shared handle. Listeners are aborted when the session ends.

use crate::commands::send_json;
use crate::store::PortForward;
use crate::{error::AppResult, ssh};
use std::sync::Arc;
use tauri::ipc::{Channel, InvokeResponseBody};

pub async fn run_listener(
    handle: Arc<ssh::SessionHandle<ssh::Client>>,
    fw: PortForward,
    events: Channel<InvokeResponseBody>,
    listener: tokio::net::TcpListener,
) -> AppResult<()> {
    tracing::info!(
        port = fw.local_port,
        remote = %format!("{}:{}", fw.remote_host, fw.remote_port),
        "local forward listening"
    );
    loop {
        let (mut tcp, peer) = listener.accept().await.map_err(crate::error::AppError::Io)?;
        let handle = handle.clone();
        let events = events.clone();
        let fw = fw.clone();
        tokio::spawn(async move {
            match handle
                .channel_open_direct_tcpip(
                    fw.remote_host.clone(),
                    fw.remote_port as u32,
                    peer.ip().to_string(),
                    peer.port() as u32,
                )
                .await
            {
                Ok(ch) => {
                    let mut stream = ch.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut stream).await;
                }
                Err(e) => {
                    tracing::warn!(error = ?e, port = fw.local_port, "forward channel open failed");
                    send_json(
                        &events,
                        serde_json::json!({"type": "forward-error", "local_port": fw.local_port}),
                    );
                }
            }
        });
    }
}

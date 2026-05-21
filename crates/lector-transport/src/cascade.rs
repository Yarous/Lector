use anyhow::Result;
use quinn::Endpoint;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use tracing::info;

use crate::certs;
use crate::receiver::ReceiveProgress;
use crate::CHUNK_SIZE;

pub struct CascadeNode {
    server_endpoint: Endpoint,
}

impl CascadeNode {
    pub fn new(bind_addr: SocketAddr) -> Result<Self> {
        Ok(Self {
            server_endpoint: crate::receiver::create_server_endpoint(bind_addr)?,
        })
    }

    pub fn from_server_endpoint(server_endpoint: Endpoint) -> Self {
        Self { server_endpoint }
    }

    pub async fn receive_and_forward(
        &self,
        dest: &Path,
        children: &[SocketAddr],
        expected_size: Option<u64>,
        progress_tx: watch::Sender<ReceiveProgress>,
    ) -> Result<[u8; 32]> {
        let mut client_endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        client_endpoint.set_default_client_config(certs::make_client_config()?);

        let mut child_streams = Vec::with_capacity(children.len());
        for &child_addr in children {
            let conn = client_endpoint.connect(child_addr, "localhost")?.await?;
            let stream = conn.open_uni().await?;
            child_streams.push((conn, stream));
        }

        let incoming = self
            .server_endpoint
            .accept()
            .await
            .ok_or_else(|| anyhow::anyhow!("endpoint closed"))?;

        let connection = incoming.await?;
        let mut recv_stream = connection.accept_uni().await?;

        let mut file = File::create(dest).await?;
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut hasher = Sha256::new();
        let mut received = 0u64;

        loop {
            let n = recv_stream.read(&mut buf).await?;
            let Some(n) = n else { break };
            if n == 0 {
                break;
            }

            let chunk = &buf[..n];
            hasher.update(chunk);
            file.write_all(chunk).await?;

            for (_, stream) in &mut child_streams {
                stream.write_all(chunk).await?;
            }

            received += n as u64;
            let _ = progress_tx.send(ReceiveProgress {
                bytes_received: received,
                total_bytes: expected_size,
            });
        }

        file.flush().await?;

        for (conn, mut stream) in child_streams {
            stream.finish()?;
            conn.close(0u32.into(), b"done");
        }

        let hash: [u8; 32] = hasher.finalize().into();
        info!(bytes = received, children = children.len(), "cascade complete");
        Ok(hash)
    }
}

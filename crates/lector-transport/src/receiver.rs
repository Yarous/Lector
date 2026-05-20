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
use crate::CHUNK_SIZE;

pub struct FileReceiver {
    endpoint: Endpoint,
}

#[derive(Debug, Clone, Default)]
pub struct ReceiveProgress {
    pub bytes_received: u64,
    pub total_bytes: Option<u64>,
}

impl FileReceiver {
    pub fn new(bind_addr: SocketAddr) -> Result<Self> {
        let pair = certs::CertPair::generate(vec!["localhost".into()])?;
        let server_config = certs::make_server_config(&pair)?;
        let endpoint = Endpoint::server(server_config, bind_addr)?;
        Ok(Self { endpoint })
    }

    pub async fn receive_file(
        &self, dest: &Path, expected_size: Option<u64>,
        progress_tx: watch::Sender<ReceiveProgress>,
    ) -> Result<[u8; 32]> {
        let incoming = self.endpoint.accept().await
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
            if n == 0 { break; }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).await?;
            received += n as u64;
            let _ = progress_tx.send(ReceiveProgress {
                bytes_received: received,
                total_bytes: expected_size,
            });
        }

        file.flush().await?;
        let hash: [u8; 32] = hasher.finalize().into();
        info!(bytes = received, "file received");
        Ok(hash)
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.endpoint.local_addr()?)
    }
}

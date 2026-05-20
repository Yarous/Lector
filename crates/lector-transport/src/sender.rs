use anyhow::Result;
use quinn::Endpoint;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tracing::info;

use crate::certs;
use crate::CHUNK_SIZE;

pub struct FileSender {
    endpoint: Endpoint,
}

impl FileSender {
    pub fn new(bind_addr: SocketAddr) -> Result<Self> {
        let pair = certs::CertPair::generate(vec!["localhost".into()])?;
        let server_config = certs::make_server_config(&pair)?;
        let mut endpoint = Endpoint::server(server_config, bind_addr)?;
        endpoint.set_default_client_config(certs::make_client_config()?);
        Ok(Self { endpoint })
    }

    pub async fn send_file(&self, path: &Path, target: SocketAddr) -> Result<[u8; 32]> {
        let connection = self.endpoint.connect(target, "localhost")?.await?;
        let mut send_stream = connection.open_uni().await?;
        let mut file = File::open(path).await?;
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut hasher = Sha256::new();
        let mut total = 0u64;

        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
            send_stream.write_all(&buf[..n]).await?;
            total += n as u64;
        }

        send_stream.finish()?;
        connection.close(0u32.into(), b"done");
        let hash: [u8; 32] = hasher.finalize().into();
        info!(bytes = total, "file sent");
        Ok(hash)
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.endpoint.local_addr()?)
    }
}

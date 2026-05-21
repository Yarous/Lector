use dashmap::DashMap;
use lector_transport::{receiver::ReceiveProgress, Endpoint};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub grpc_port: u16,
    pub quic_port: u16,
    pub download_dir: PathBuf,
    #[serde(default)]
    pub teacher_addr: Option<String>,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("LECTOR_CONFIG").unwrap_or_else(|_| {
            if cfg!(windows) {
                r"C:\ProgramData\Lector\config.toml".into()
            } else {
                "/etc/lector/config.toml".into()
            }
        });

        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(toml::from_str(&content)?),
            Err(_) => Ok(Self::default()),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            grpc_port: 50051,
            quic_port: 50052,
            download_dir: if cfg!(windows) {
                PathBuf::from(r"C:\ProgramData\Lector\Downloads")
            } else {
                PathBuf::from("/var/lib/lector/downloads")
            },
            teacher_addr: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransferState {
    pub file_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub parent: SocketAddr,
    pub children: Vec<SocketAddr>,
    pub progress_rx: watch::Receiver<ReceiveProgress>,
}

#[derive(Clone)]
pub struct DaemonState {
    pub config: Config,
    pub quic_endpoint: Endpoint,
    pub transfers: Arc<DashMap<String, TransferState>>,
    pub active_transfer: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl DaemonState {
    pub fn new(config: Config, quic_endpoint: Endpoint) -> Self {
        Self {
            config,
            quic_endpoint,
            transfers: Arc::new(DashMap::new()),
            active_transfer: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

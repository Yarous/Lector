use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

use lector_transport::receiver::ReceiveProgress;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    pub(crate) grpc_port: u16,
    pub(crate) quic_port: u16,
    pub(crate) download_dir: PathBuf,
    #[serde(default)]
    pub(crate) teacher_addr: Option<String>,
}

impl Config {
    pub(crate) fn load() -> anyhow::Result<Self> {
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

#[allow(unused)]
#[derive(Debug, Clone)]
pub(crate) struct TransferState {
    pub(crate) file_id: String,
    pub(crate) file_name: String,
    pub(crate) file_size: u64,
    pub(crate) parent: SocketAddr,
    pub(crate) children: Vec<SocketAddr>,
    pub(crate) progress_rx: watch::Receiver<ReceiveProgress>,
}

#[derive(Debug, Clone)]
pub struct DaemonState {
    pub config: Config,
    pub transfers: Arc<DashMap<String, TransferState>>,
    pub active_transfer: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl DaemonState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            transfers: Arc::new(DashMap::new()),
            active_transfer: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

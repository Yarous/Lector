use lector_proto::{LectorDaemonClient, PingRequest};
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;
use tonic::transport::Channel;

#[derive(Debug, Clone)]
pub struct PeerScanResult {
    pub addr: SocketAddr,
    pub online: bool,
    pub hostname: String,
    pub ping_ms: u64,
    pub free_disk_gb: f64,
    pub version: String,
}

pub async fn scan_network(peers: &[SocketAddr]) -> Vec<PeerScanResult> {
    let handles: Vec<_> = peers.iter().map(|&addr| tokio::spawn(scan_peer(addr))).collect();
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }
    results.sort_by(|a, b| {
        b.online.cmp(&a.online)
            .then(a.ping_ms.cmp(&b.ping_ms))
            .then(a.addr.cmp(&b.addr))
    });
    results
}

async fn scan_peer(addr: SocketAddr) -> PeerScanResult {
    let url = format!("http://{}", addr);
    let start = Instant::now();
    let result = timeout(Duration::from_secs(3), async {
        let channel = Channel::from_shared(url)?
            .connect_timeout(Duration::from_secs(2))
            .connect()
            .await?;
        let mut client = LectorDaemonClient::new(channel);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let response = client.ping(PingRequest { timestamp: now }).await?;
        Ok::<_, anyhow::Error>(response.into_inner())
    })
    .await;

    let elapsed = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(resp)) => PeerScanResult {
            addr,
            online: true,
            hostname: resp.hostname,
            ping_ms: elapsed,
            free_disk_gb: resp.free_disk_bytes as f64 / 1_073_741_824.0,
            version: resp.daemon_version,
        },
        _ => PeerScanResult {
            addr,
            online: false,
            hostname: String::new(),
            ping_ms: 0,
            free_disk_gb: 0.0,
            version: String::new(),
        },
    }
}

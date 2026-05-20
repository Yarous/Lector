use lector_proto::{LectorDaemonClient, TelemetryRequest};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;
use tonic::transport::Channel;

#[derive(Debug, Clone)]
pub struct NodeTelemetry {
    pub addr: SocketAddr,
    pub progress: u32,
    pub speed_bps: u64,
    pub reachable: bool,
}

pub async fn poll_telemetry(peers: &[SocketAddr]) -> HashMap<SocketAddr, NodeTelemetry> {
    let handles: Vec<_> = peers.iter().map(|&addr| {
        tokio::spawn(async move {
            let url = format!("http://{}", addr);
            let result = timeout(Duration::from_secs(2), async {
                let channel = Channel::from_shared(url)?.connect().await?;
                let mut client = LectorDaemonClient::new(channel);
                let resp = client.get_telemetry(TelemetryRequest {}).await?;
                Ok::<_, anyhow::Error>(resp.into_inner())
            }).await;

            let telemetry = match result {
                Ok(Ok(resp)) => NodeTelemetry {
                    addr,
                    progress: resp.progress_percent,
                    speed_bps: resp.download_speed_bps,
                    reachable: true,
                },
                _ => NodeTelemetry {
                    addr,
                    progress: 0,
                    speed_bps: 0,
                    reachable: false,
                },
            };
            (addr, telemetry)
        })
    }).collect();

    let mut results = HashMap::with_capacity(handles.len());
    for handle in handles {
        if let Ok((addr, telemetry)) = handle.await {
            results.insert(addr, telemetry);
        }
    }
    results
}

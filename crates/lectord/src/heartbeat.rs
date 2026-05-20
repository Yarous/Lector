use std::net::SocketAddr;
use std::time::Duration;
use tokio::time;
use tonic::transport::Channel;
use tracing::warn;

use lector_proto::{LectorDaemonClient, PingRequest};

pub async fn start_heartbeat(teacher_addr: SocketAddr, interval: Duration) {
    let url = format!("http://{}", teacher_addr);
    loop {
        time::sleep(interval).await;
        match Channel::from_shared(url.clone()) {
            Ok(channel) => match channel.connect().await {
                Ok(conn) => {
                    let mut client = LectorDaemonClient::new(conn);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    if let Err(e) = client.ping(PingRequest { timestamp: now }).await {
                        warn!(error = %e, "heartbeat ping failed");
                    }
                }
                Err(e) => warn!(error = %e, "heartbeat connection failed"),
            },
            Err(e) => warn!(error = %e, "invalid teacher address"),
        }
    }
}

mod grpc_server;
mod heartbeat;
mod state;
mod transfer;
mod service;

use anyhow::Result;
use lector_transport::receiver::create_server_endpoint;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

use crate::state::DaemonState;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("lectord=info".parse()?))
        .init();

    let config = state::Config::load()?;
    let quic_addr: SocketAddr = format!("0.0.0.0:{}", config.quic_port).parse()?;
    let quic_endpoint = create_server_endpoint(quic_addr)?;
    let state = DaemonState::new(config.clone(), quic_endpoint);

    let grpc_addr: SocketAddr = format!("0.0.0.0:{}", config.grpc_port).parse()?;

    tracing::info!(%grpc_addr, %quic_addr, "starting lectord");

    grpc_server::serve(grpc_addr, state).await?;

    Ok(())
}

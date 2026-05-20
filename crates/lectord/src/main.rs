mod grpc_server;
mod heartbeat;
mod state;
mod transfer;
mod service;

use anyhow::Result;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

use crate::state::DaemonState;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("lectord=info".parse()?))
        .init();

    let config = state::Config::load()?;
    let state = DaemonState::new(config.clone());
    let grpc_addr: SocketAddr = format!("0.0.0.0:{}", config.grpc_port).parse()?;
    tracing::info!(%grpc_addr, "starting lectord");
    grpc_server::serve(grpc_addr, state).await?;
    Ok(())
}

use anyhow::{bail, Result};
use std::net::SocketAddr;
use tokio::sync::watch;

use lector_proto::DownloadInstruction;
use lector_transport::cascade::CascadeNode;
use lector_transport::receiver::{FileReceiver, ReceiveProgress};

use crate::state::{DaemonState, TransferState};

pub async fn execute(state: DaemonState, instruction: DownloadInstruction) -> Result<()> {
    {
        let mut active = state.active_transfer.lock().await;
        if active.is_some() {
            bail!("transfer already in progress");
        }
        *active = Some(instruction.file_id.clone());
    }

    let result = execute_inner(state.clone(), instruction.clone()).await;

    {
        let mut active = state.active_transfer.lock().await;
        if active.as_deref() == Some(instruction.file_id.as_str()) {
            *active = None;
        }
    }

    if result.is_err() {
        state.transfers.remove(&instruction.file_id);
    }

    result
}

async fn execute_inner(state: DaemonState, instruction: DownloadInstruction) -> Result<()> {
    let dest = state.config.download_dir.join(&instruction.file_name);

    tokio::fs::create_dir_all(&state.config.download_dir).await?;

    let parent_addr: SocketAddr = instruction.parent_address.parse()?;
    let children: Vec<SocketAddr> = instruction
        .children_addresses
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();

    let (progress_tx, progress_rx) = watch::channel(ReceiveProgress::default());

    let transfer = TransferState {
        file_id: instruction.file_id.clone(),
        file_name: instruction.file_name.clone(),
        file_size: instruction.file_size,
        parent: parent_addr,
        children: children.clone(),
        progress_rx,
    };

    state.transfers.insert(instruction.file_id.clone(), transfer);

    let expected_size = Some(instruction.file_size);
    let server_endpoint = state.quic_endpoint.clone();

    let hash = if children.is_empty() {
        let receiver = FileReceiver::from_endpoint(server_endpoint);
        receiver
            .receive_file(&dest, expected_size, progress_tx)
            .await?
    } else {
        let node = CascadeNode::from_server_endpoint(server_endpoint);
        node.receive_and_forward(&dest, &children, expected_size, progress_tx)
            .await?
    };

    tracing::info!(
        file = %instruction.file_name,
        hash = %hex::encode(hash),
        "transfer complete"
    );

    Ok(())
}

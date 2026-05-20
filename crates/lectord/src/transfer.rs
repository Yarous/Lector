use anyhow::Result;
use std::net::SocketAddr;
use tokio::sync::watch;

use lector_proto::DownloadInstruction;
use lector_transport::cascade::CascadeNode;
use lector_transport::receiver::{FileReceiver, ReceiveProgress};

use crate::state::{DaemonState, TransferState};

pub async fn execute(state: DaemonState, instruction: DownloadInstruction) -> Result<()> {
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
    {
        let mut active = state.active_transfer.lock().await;
        *active = Some(instruction.file_id.clone());
    }

    let bind_addr: SocketAddr = format!("0.0.0.0:{}", state.config.quic_port).parse()?;
    let expected_size = Some(instruction.file_size);

    let hash = if children.is_empty() {
        let receiver = FileReceiver::new(bind_addr)?;
        receiver.receive_file(&dest, expected_size, progress_tx).await?
    } else {
        let node = CascadeNode::new(bind_addr)?;
        node.receive_and_forward(&dest, &children, expected_size, progress_tx).await?
    };

    tracing::info!(
        file = %instruction.file_name,
        hash = %hex::encode(hash),
        "transfer complete"
    );

    {
        let mut active = state.active_transfer.lock().await;
        *active = None;
    }

    Ok(())
}

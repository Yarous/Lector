use slint::ComponentHandle;

use lector_proto::{DownloadInstruction, LectorDaemonClient};
use lector_topology::DistributionTree;
use lector_transport::sender::FileSender;
use lector_transport::QUIC_PORT;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use tokio::fs;
use tonic::transport::Channel;

use crate::app_state::AppState;
use crate::MainWindow;

pub async fn distribute(state: AppState, ui: slint::Weak<MainWindow>) {
    let file_path = match state.selected_file() {
        Some(p) => p,
        None => {
            set_status(&ui, "No file selected");
            set_distributing(&ui, false);
            return;
        }
    };

    let selected_peers = state.selected_peer_addresses();
    if selected_peers.is_empty() {
        set_status(&ui, "No peers selected");
        set_distributing(&ui, false);
        return;
    }

    set_status(&ui, &format!("Preparing distribution to {} peers...", selected_peers.len()));

    let file_meta = match fs::metadata(&file_path).await {
        Ok(m) => m,
        Err(e) => {
            set_status(&ui, &format!("Cannot read file: {}", e));
            set_distributing(&ui, false);
            return;
        }
    };

    let file_size = file_meta.len();
    let file_name = file_path.file_name().unwrap().to_string_lossy().to_string();

    set_status(&ui, "Computing file hash...");

    let file_bytes = match fs::read(&file_path).await {
        Ok(b) => b,
        Err(e) => {
            set_status(&ui, &format!("Read error: {}", e));
            set_distributing(&ui, false);
            return;
        }
    };

    let file_hash: [u8; 32] = Sha256::digest(&file_bytes).into();
    let file_id = hex::encode(&file_hash[..8]);
    let hash_display = hex::encode(&file_hash[..16]) + "…";

    set_hash(&ui, &hash_display);

    let teacher_quic: SocketAddr = "0.0.0.0:50052".parse().unwrap();
    let quic_peers: Vec<SocketAddr> = selected_peers
        .iter()
        .map(|a| SocketAddr::new(a.ip(), QUIC_PORT))
        .collect();

    let tree = DistributionTree::build(teacher_quic, &quic_peers, 2);

    set_status(&ui, &format!(
        "Tree built: {} nodes, depth ~{}",
        tree.nodes.len(),
        (quic_peers.len() as f64).log2().ceil() as u32 + 1
    ));

    set_status(&ui, "Sending download instructions...");

    let mut instruction_errors = 0u32;

    for node in &tree.nodes {
        if node.addr == teacher_quic {
            continue;
        }

        let grpc_addr = SocketAddr::new(node.addr.ip(), 50051);
        let parent = node.parent.unwrap_or(teacher_quic);
        let children: Vec<String> = node.children.iter().map(|c| c.to_string()).collect();

        let instruction = DownloadInstruction {
            file_id: file_id.clone(),
            file_name: file_name.clone(),
            file_size,
            file_hash: file_hash.to_vec(),
            parent_address: parent.to_string(),
            children_addresses: children,
        };

        if let Err(e) = send_instruction(grpc_addr, instruction).await {
            tracing::error!(%grpc_addr, error = %e, "instruction failed");
            instruction_errors += 1;
        }
    }

    if instruction_errors > 0 {
        set_status(&ui, &format!("⚠ {} peers unreachable, sending to remaining...", instruction_errors));
    } else {
        set_status(&ui, "All peers ready — streaming file...");
    }

    let root_children: Vec<SocketAddr> = tree
        .find_node(teacher_quic)
        .map(|n| n.children.clone())
        .unwrap_or_default();

    match FileSender::new(teacher_quic) {
        Ok(sender) => {
            let total_children = root_children.len();

            for (i, child) in root_children.iter().enumerate() {
                set_status(&ui, &format!(
                    "Streaming to root child {}/{} ({})",
                    i + 1,
                    total_children,
                    child
                ));

                if let Err(e) = sender.send_file(&file_path, *child).await {
                    tracing::error!(%child, error = %e, "send failed");
                    set_status(&ui, &format!("⚠ Failed to send to {}: {}", child, e));
                }

                let progress = ((i + 1) as f64 / total_children as f64 * 100.0) as i32;
                set_progress(&ui, progress);
            }
        }
        Err(e) => {
            set_status(&ui, &format!("QUIC init error: {}", e));
            set_distributing(&ui, false);
            return;
        }
    }

    set_progress(&ui, 100);
    set_status(&ui, &format!(
        "✅ Distribution complete — {} sent to {} peers",
        file_name,
        selected_peers.len()
    ));
    set_distributing(&ui, false);
}

async fn send_instruction(addr: SocketAddr, instruction: DownloadInstruction) -> anyhow::Result<()> {
    let url = format!("http://{}", addr);
    let channel = Channel::from_shared(url)?
        .connect_timeout(std::time::Duration::from_secs(3))
        .connect()
        .await?;
    let mut client = LectorDaemonClient::new(channel);
    client.init_download(instruction).await?;
    Ok(())
}

fn set_status(ui: &slint::Weak<MainWindow>, msg: &str) {
    let msg = msg.to_string();
    let ui = ui.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            ui.global::<crate::AppBridge>().set_status_text(msg.into());
        }
    }).ok();
}

fn set_distributing(ui: &slint::Weak<MainWindow>, val: bool) {
    let ui = ui.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            ui.global::<crate::AppBridge>().set_distributing(val);
        }
    }).ok();
}

fn set_progress(ui: &slint::Weak<MainWindow>, val: i32) {
    let ui = ui.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            ui.global::<crate::AppBridge>().set_overall_progress(val);
        }
    }).ok();
}

fn set_hash(ui: &slint::Weak<MainWindow>, hash: &str) {
    let hash = hash.to_string();
    let ui = ui.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            ui.global::<crate::AppBridge>().set_file_hash(hash.into());
        }
    }).ok();
}

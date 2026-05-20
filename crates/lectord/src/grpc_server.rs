use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tonic::{Request, Response, Status};

use lector_proto::*;
use crate::state::{DaemonState, VERSION};
use crate::transfer;

pub struct DaemonService {
    state: DaemonState,
}

impl DaemonService {
    fn new(state: DaemonState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl LectorDaemon for DaemonService {
    async fn ping(&self, _req: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_default();
        let free = fs2::available_space(&self.state.config.download_dir).unwrap_or(0);
        Ok(Response::new(PingResponse {
            timestamp: now,
            daemon_version: VERSION.into(),
            free_disk_bytes: free,
            hostname,
        }))
    }

    async fn init_download(
        &self, req: Request<DownloadInstruction>,
    ) -> Result<Response<ActionResponse>, Status> {
        let instruction = req.into_inner();
        let state = self.state.clone();
        tokio::spawn(async move {
            if let Err(e) = transfer::execute(state, instruction).await {
                tracing::error!(error = %e, "transfer failed");
            }
        });
        Ok(Response::new(ActionResponse {
            success: true,
            error_message: String::new(),
        }))
    }

    async fn update_topology(
        &self, req: Request<TopologyInstruction>,
    ) -> Result<Response<ActionResponse>, Status> {
        let instruction = req.into_inner();
        tracing::info!(
            file_id = %instruction.file_id,
            new_parent = %instruction.new_parent_address,
            "topology update received"
        );
        Ok(Response::new(ActionResponse {
            success: true,
            error_message: String::new(),
        }))
    }

    async fn get_telemetry(
        &self, _req: Request<TelemetryRequest>,
    ) -> Result<Response<TelemetryResponse>, Status> {
        let active = self.state.active_transfer.lock().await;
        let (progress, file_id) = match active.as_ref() {
            Some(id) => match self.state.transfers.get(id) {
                Some(t) => {
                    let p = t.progress_rx.borrow().clone();
                    let pct = match p.total_bytes {
                        Some(total) if total > 0 => {
                            ((p.bytes_received as f64 / total as f64) * 100.0) as u32
                        }
                        _ => 0,
                    };
                    (pct, id.clone())
                }
                None => (0, String::new()),
            },
            None => (0, String::new()),
        };
        Ok(Response::new(TelemetryResponse {
            progress_percent: progress,
            download_speed_bps: 0,
            cpu_usage: 0.0,
            ram_usage: 0.0,
            current_file_id: file_id,
        }))
    }

    async fn cancel_transfer(
        &self, req: Request<CancelRequest>,
    ) -> Result<Response<ActionResponse>, Status> {
        let file_id = req.into_inner().file_id;
        self.state.transfers.remove(&file_id);
        Ok(Response::new(ActionResponse {
            success: true,
            error_message: String::new(),
        }))
    }
}

pub async fn serve(addr: SocketAddr, state: DaemonState) -> anyhow::Result<()> {
    let service = DaemonService::new(state);
    let server = LectorDaemonServer::new(service);
    tracing::info!(%addr, "gRPC server listening");
    tonic::transport::Server::builder()
        .add_service(server)
        .serve(addr)
        .await?;
    Ok(())
}

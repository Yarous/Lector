pub mod pb {
    tonic::include_proto!("lector");
}

pub use pb::*;
pub use pb::lector_daemon_client::LectorDaemonClient;
pub use pb::lector_daemon_server::{LectorDaemon, LectorDaemonServer};

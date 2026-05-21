#[cfg(windows)]
pub mod windows_service {
    use lector_transport::receiver::create_server_endpoint;
    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };

    const SERVICE_NAME: &str = "LectorDaemon";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    define_windows_service!(ffi_service_main, service_main);

    pub fn run() -> windows_service::Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
    }

    fn service_main(_arguments: Vec<std::ffi::OsString>) {
        let status_handle = service_control_handler::register(
            SERVICE_NAME,
            |control| match control {
                ServiceControl::Stop => ServiceControlHandlerResult::NoError,
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            },
        )
        .unwrap();

        status_handle
            .set_service_status(ServiceStatus {
                service_type: SERVICE_TYPE,
                current_state: ServiceState::Running,
                controls_accepted: ServiceControlAccept::STOP,
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: std::time::Duration::default(),
                process_id: None,
            })
            .unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = crate::state::Config::load().unwrap();
            let quic_addr = format!("0.0.0.0:{}", config.quic_port).parse().unwrap();
            let quic_endpoint = create_server_endpoint(quic_addr).unwrap();
            let state = crate::state::DaemonState::new(config.clone(), quic_endpoint);
            let addr = format!("0.0.0.0:{}", config.grpc_port).parse().unwrap();
            crate::grpc_server::serve(addr, state).await.unwrap();
        });
    }
}

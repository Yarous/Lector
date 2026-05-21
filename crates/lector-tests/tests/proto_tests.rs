use lector_proto::*;

#[test]
fn ping_request_default() {
    let req = PingRequest::default();

    assert_eq!(req.timestamp, 0);
}

#[test]
fn ping_response_fields() {
    let resp = PingResponse {
        timestamp: 1234567890,
        daemon_version: "0.1.0".into(),
        free_disk_bytes: 1024 * 1024 * 1024,
        hostname: "test-pc".into(),
    };

    assert_eq!(resp.timestamp, 1234567890);
    assert_eq!(resp.daemon_version, "0.1.0");
    assert_eq!(resp.free_disk_bytes, 1_073_741_824);
    assert_eq!(resp.hostname, "test-pc");
}

#[test]
fn download_instruction_construction() {
    let instruction = DownloadInstruction {
        file_id: "abc123".into(),
        file_name: "test.pdf".into(),
        file_size: 1048576,
        file_hash: vec![0xDE, 0xAD, 0xBE, 0xEF],
        parent_address: "192.168.1.5:50052".into(),
        children_addresses: vec![
            "192.168.1.10:50052".into(),
            "192.168.1.11:50052".into(),
        ],
    };

    assert_eq!(instruction.file_id, "abc123");
    assert_eq!(instruction.file_name, "test.pdf");
    assert_eq!(instruction.file_size, 1048576);
    assert_eq!(instruction.file_hash, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(instruction.children_addresses.len(), 2);
}

#[test]
fn action_response_success() {
    let resp = ActionResponse {
        success: true,
        error_message: String::new(),
    };

    assert!(resp.success);
    assert!(resp.error_message.is_empty());
}

#[test]
fn action_response_failure() {
    let resp = ActionResponse {
        success: false,
        error_message: "connection refused".into(),
    };

    assert!(!resp.success);
    assert_eq!(resp.error_message, "connection refused");
}

#[test]
fn telemetry_response_fields() {
    let resp = TelemetryResponse {
        progress_percent: 75,
        download_speed_bps: 10_000_000,
        cpu_usage: 45.5,
        ram_usage: 62.3,
        current_file_id: "file-001".into(),
    };

    assert_eq!(resp.progress_percent, 75);
    assert_eq!(resp.download_speed_bps, 10_000_000);
    assert!((resp.cpu_usage - 45.5).abs() < f32::EPSILON);
    assert!((resp.ram_usage - 62.3).abs() < f32::EPSILON);
}

#[test]
fn topology_instruction_construction() {
    let instruction = TopologyInstruction {
        file_id: "transfer-42".into(),
        new_parent_address: "192.168.1.100:50052".into(),
        new_children_addresses: vec!["192.168.1.200:50052".into()],
    };

    assert_eq!(instruction.file_id, "transfer-42");
    assert_eq!(instruction.new_parent_address, "192.168.1.100:50052");
    assert_eq!(instruction.new_children_addresses.len(), 1);
}

#[test]
fn cancel_request_construction() {
    let req = CancelRequest {
        file_id: "cancel-me".into(),
    };

    assert_eq!(req.file_id, "cancel-me");
}

#[test]
fn protobuf_roundtrip() {
    use prost::Message;

    let original = DownloadInstruction {
        file_id: "roundtrip".into(),
        file_name: "document.pdf".into(),
        file_size: 999999,
        file_hash: vec![1, 2, 3, 4, 5],
        parent_address: "10.0.0.1:50052".into(),
        children_addresses: vec!["10.0.0.2:50052".into(), "10.0.0.3:50052".into()],
    };

    let encoded = original.encode_to_vec();
    let decoded = DownloadInstruction::decode(encoded.as_slice()).unwrap();

    assert_eq!(original.file_id, decoded.file_id);
    assert_eq!(original.file_name, decoded.file_name);
    assert_eq!(original.file_size, decoded.file_size);
    assert_eq!(original.file_hash, decoded.file_hash);
    assert_eq!(original.parent_address, decoded.parent_address);
    assert_eq!(original.children_addresses, decoded.children_addresses);
}

#[test]
fn protobuf_encoding_is_compact() {
    use prost::Message;

    let msg = PingRequest { timestamp: 1234567890 };
    let encoded = msg.encode_to_vec();

    assert!(encoded.len() < 20, "protobuf should be compact, got {} bytes", encoded.len());
}

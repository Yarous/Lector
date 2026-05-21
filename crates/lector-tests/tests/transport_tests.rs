mod common;

use common::{create_test_file, free_addr, progress_channel, TempDir};
use lector_transport::certs::CertPair;
use lector_transport::receiver::FileReceiver;
use lector_transport::sender::FileSender;
use sha2::{Digest, Sha256};

#[test]
fn cert_generation_produces_valid_pair() {
    let pair = CertPair::generate(vec!["localhost".into()]).unwrap();

    assert!(!pair.cert.is_empty());
    assert_eq!(pair.cert.len(), 1);
}

#[test]
fn cert_generation_with_multiple_sans() {
    let pair = CertPair::generate(vec![
        "localhost".into(),
        "lector.local".into(),
    ])
    .unwrap();

    assert_eq!(pair.cert.len(), 1);
}

#[test]
fn server_config_from_cert_pair() {
    let pair = CertPair::generate(vec!["localhost".into()]).unwrap();
    let config = lector_transport::certs::make_server_config(&pair);

    assert!(config.is_ok());
}

#[test]
fn client_config_creation() {
    let config = lector_transport::certs::make_client_config();

    assert!(config.is_ok());
}

#[tokio::test]
async fn send_and_receive_small_file() {
    let dir = TempDir::new("small");
    let src = dir.file("source.bin");
    let dst = dir.file("received.bin");

    let original_data = create_test_file(&src, 1024);

    let recv_addr = free_addr();
    let receiver = FileReceiver::new(recv_addr).unwrap();

    let (progress_tx, _progress_rx) = progress_channel();

    let recv_handle = tokio::spawn(async move {
        receiver
            .receive_file(&dst, Some(1024), progress_tx)
            .await
            .unwrap()
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let sender = FileSender::new(free_addr()).unwrap();
    let send_hash = sender.send_file(&src, recv_addr).await.unwrap();

    let recv_hash = recv_handle.await.unwrap();

    assert_eq!(send_hash, recv_hash);

    let expected_hash: [u8; 32] = Sha256::digest(&original_data).into();
    assert_eq!(send_hash, expected_hash);
}

#[tokio::test]
async fn send_and_receive_large_file() {
    let dir = TempDir::new("large");
    let src = dir.file("big.bin");
    let dst = dir.file("big_received.bin");

    let size = 5 * 1024 * 1024;
    let original_data = create_test_file(&src, size);

    let recv_addr = free_addr();
    let receiver = FileReceiver::new(recv_addr).unwrap();

    let (progress_tx, _progress_rx) = progress_channel();

    let recv_handle = tokio::spawn(async move {
        receiver
            .receive_file(&dst, Some(size as u64), progress_tx)
            .await
            .unwrap()
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let sender = FileSender::new(free_addr()).unwrap();
    let send_hash = sender.send_file(&src, recv_addr).await.unwrap();

    let recv_hash = recv_handle.await.unwrap();

    assert_eq!(send_hash, recv_hash);

    let expected: [u8; 32] = Sha256::digest(&original_data).into();
    assert_eq!(send_hash, expected);
}

#[tokio::test]
async fn progress_tracking_works() {
    let dir = TempDir::new("progress");
    let src = dir.file("progress_src.bin");
    let dst = dir.file("progress_dst.bin");

    let size = 256 * 1024;
    create_test_file(&src, size);

    let recv_addr = free_addr();
    let receiver = FileReceiver::new(recv_addr).unwrap();

    let (progress_tx, mut progress_rx) = progress_channel();

    let recv_handle = tokio::spawn(async move {
        receiver
            .receive_file(&dst, Some(size as u64), progress_tx)
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let progress_handle = tokio::spawn(async move {
        let mut max_bytes = 0u64;
        loop {
            if progress_rx.changed().await.is_err() {
                break;
            }
            let p = progress_rx.borrow().clone();
            if p.bytes_received > max_bytes {
                max_bytes = p.bytes_received;
            }
            if p.bytes_received >= size as u64 {
                break;
            }
        }
        max_bytes
    });

    let sender = FileSender::new(free_addr()).unwrap();
    sender.send_file(&src, recv_addr).await.unwrap();

    recv_handle.await.unwrap().unwrap();
    let final_bytes = progress_handle.await.unwrap();

    assert_eq!(final_bytes, size as u64);
}

#[tokio::test]
async fn sender_reports_correct_local_addr() {
    let bind = free_addr();
    let sender = FileSender::new(bind).unwrap();
    let local = sender.local_addr().unwrap();

    assert_eq!(local.port(), bind.port());
}

#[tokio::test]
async fn receiver_reports_correct_local_addr() {
    let bind = free_addr();
    let receiver = FileReceiver::new(bind).unwrap();
    let local = receiver.local_addr().unwrap();

    assert_eq!(local.port(), bind.port());
}

#[tokio::test]
async fn transfer_empty_file() {
    let dir = TempDir::new("empty");
    let src = dir.file("empty.bin");
    let dst = dir.file("empty_recv.bin");

    create_test_file(&src, 0);

    let recv_addr = free_addr();
    let receiver = FileReceiver::new(recv_addr).unwrap();
    let (progress_tx, _) = progress_channel();

    let recv_handle = tokio::spawn(async move {
        receiver
            .receive_file(&dst, Some(0), progress_tx)
            .await
            .unwrap()
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let sender = FileSender::new(free_addr()).unwrap();
    let send_hash = sender.send_file(&src, recv_addr).await.unwrap();

    let recv_hash = recv_handle.await.unwrap();
    assert_eq!(send_hash, recv_hash);
}

#[tokio::test]
async fn received_file_matches_original() {
    let dir = TempDir::new("content");
    let src = dir.file("original.txt");
    let dst = dir.file("copy.txt");

    let data = create_test_file(&src, 4096);

    let recv_addr = free_addr();
    let receiver = FileReceiver::new(recv_addr).unwrap();
    let (progress_tx, _) = progress_channel();

    let dst_clone = dst.clone();
    let recv_handle = tokio::spawn(async move {
        receiver
            .receive_file(&dst_clone, Some(4096), progress_tx)
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let sender = FileSender::new(free_addr()).unwrap();
    sender.send_file(&src, recv_addr).await.unwrap();

    recv_handle.await.unwrap().unwrap();

    let received_data = std::fs::read(&dst).unwrap();
    assert_eq!(data, received_data);
}

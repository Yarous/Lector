mod common;

use common::{create_test_file, free_addr, progress_channel, TempDir};
use lector_transport::cascade::CascadeNode;
use lector_transport::receiver::FileReceiver;
use lector_transport::sender::FileSender;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::time::Duration;

async fn setup_leaf(addr: SocketAddr, dest: std::path::PathBuf, size: u64) -> tokio::task::JoinHandle<[u8; 32]> {
    let (tx, _) = progress_channel();
    tokio::spawn(async move {
        let receiver = FileReceiver::new(addr).unwrap();
        receiver
            .receive_file(&dest, Some(size), tx)
            .await
            .unwrap()
    })
}

async fn setup_cascade(
    bind: SocketAddr,
    dest: std::path::PathBuf,
    children: Vec<SocketAddr>,
    size: u64,
) -> tokio::task::JoinHandle<[u8; 32]> {
    let (tx, _) = progress_channel();
    tokio::spawn(async move {
        let node = CascadeNode::new(bind).unwrap();
        node.receive_and_forward(&dest, &children, Some(size), tx)
            .await
            .unwrap()
    })
}

#[tokio::test]
async fn cascade_to_single_child() {
    let dir = TempDir::new("cascade1");
    let src = dir.file("src.bin");
    let mid = dir.file("mid.bin");
    let dst = dir.file("dst.bin");

    let size = 128 * 1024;
    let data = create_test_file(&src, size);
    let expected: [u8; 32] = Sha256::digest(&data).into();

    let cascade_addr = free_addr();
    let leaf_addr = free_addr();

    let leaf_handle = setup_leaf(leaf_addr, dst, size as u64).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let cascade_handle = setup_cascade(
        cascade_addr,
        mid,
        vec![leaf_addr],
        size as u64,
    ).await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let sender = FileSender::new(free_addr()).unwrap();
    let send_hash = sender.send_file(&src, cascade_addr).await.unwrap();

    let cascade_hash = tokio::time::timeout(Duration::from_secs(10), cascade_handle)
        .await
        .expect("cascade timed out")
        .expect("cascade panicked");

    let leaf_hash = tokio::time::timeout(Duration::from_secs(10), leaf_handle)
        .await
        .expect("leaf timed out")
        .expect("leaf panicked");

    assert_eq!(send_hash, expected);
    assert_eq!(cascade_hash, expected);
    assert_eq!(leaf_hash, expected);
}

#[tokio::test]
async fn cascade_to_two_children() {
    let dir = TempDir::new("cascade2");
    let src = dir.file("src.bin");
    let mid = dir.file("mid.bin");
    let dst1 = dir.file("dst1.bin");
    let dst2 = dir.file("dst2.bin");

    let size = 64 * 1024;
    let data = create_test_file(&src, size);
    let expected: [u8; 32] = Sha256::digest(&data).into();

    let cascade_addr = free_addr();
    let leaf1_addr = free_addr();
    let leaf2_addr = free_addr();

    let h1 = setup_leaf(leaf1_addr, dst1, size as u64).await;
    let h2 = setup_leaf(leaf2_addr, dst2, size as u64).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let ch = setup_cascade(
        cascade_addr,
        mid,
        vec![leaf1_addr, leaf2_addr],
        size as u64,
    ).await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let sender = FileSender::new(free_addr()).unwrap();
    sender.send_file(&src, cascade_addr).await.unwrap();

    let cascade_hash = tokio::time::timeout(Duration::from_secs(10), ch)
        .await
        .expect("cascade timed out")
        .expect("cascade panicked");

    let hash1 = tokio::time::timeout(Duration::from_secs(10), h1)
        .await
        .expect("leaf1 timed out")
        .expect("leaf1 panicked");

    let hash2 = tokio::time::timeout(Duration::from_secs(10), h2)
        .await
        .expect("leaf2 timed out")
        .expect("leaf2 panicked");

    assert_eq!(cascade_hash, expected);
    assert_eq!(hash1, expected);
    assert_eq!(hash2, expected);
}

#[tokio::test]
async fn cascade_preserves_file_content() {
    let dir = TempDir::new("cascade-content");
    let src = dir.file("src.bin");
    let mid = dir.file("mid.bin");
    let dst = dir.file("dst.bin");

    let size = 32 * 1024;
    let original = create_test_file(&src, size);

    let cascade_addr = free_addr();
    let leaf_addr = free_addr();

    let dst_clone = dst.clone();
    let lh = setup_leaf(leaf_addr, dst_clone, size as u64).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mid_clone = mid.clone();
    let ch = setup_cascade(
        cascade_addr,
        mid_clone,
        vec![leaf_addr],
        size as u64,
    ).await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let sender = FileSender::new(free_addr()).unwrap();
    sender.send_file(&src, cascade_addr).await.unwrap();

    tokio::time::timeout(Duration::from_secs(10), ch)
        .await
        .expect("cascade timed out")
        .expect("cascade panicked");

    tokio::time::timeout(Duration::from_secs(10), lh)
        .await
        .expect("leaf timed out")
        .expect("leaf panicked");

    let mid_data = std::fs::read(&mid).unwrap();
    let dst_data = std::fs::read(&dst).unwrap();

    assert_eq!(original, mid_data);
    assert_eq!(original, dst_data);
}

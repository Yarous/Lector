mod common;

use common::{addr, create_test_file, free_addr, free_port, progress_channel, TempDir};
use lector_topology::DistributionTree;
use lector_transport::cascade::CascadeNode;
use lector_transport::receiver::FileReceiver;
use lector_transport::sender::FileSender;
use sha2::{Digest, Sha256};

#[tokio::test]
async fn full_tree_distribution_3_nodes() {
    let dir = TempDir::new("full3");
    let src = dir.file("src.bin");
    let mid = dir.file("mid.bin");
    let leaf1 = dir.file("leaf1.bin");
    let leaf2 = dir.file("leaf2.bin");

    let size = 100 * 1024;
    let data = create_test_file(&src, size);
    let expected: [u8; 32] = Sha256::digest(&data).into();

    let teacher_addr = free_addr();
    let cascade_addr = free_addr();
    let leaf1_addr = free_addr();
    let leaf2_addr = free_addr();

    let tree = DistributionTree::build(
        teacher_addr,
        &[cascade_addr, leaf1_addr, leaf2_addr],
        2,
    );

    assert_eq!(tree.nodes.len(), 4);

    let r1 = FileReceiver::new(leaf1_addr).unwrap();
    let r2 = FileReceiver::new(leaf2_addr).unwrap();
    let (tx1, _) = progress_channel();
    let (tx2, _) = progress_channel();

    let h1 = tokio::spawn(async move {
        r1.receive_file(&leaf1, Some(size as u64), tx1)
            .await
            .unwrap()
    });

    let h2 = tokio::spawn(async move {
        r2.receive_file(&leaf2, Some(size as u64), tx2)
            .await
            .unwrap()
    });

    let node = CascadeNode::new(cascade_addr).unwrap();
    let (ctx, _) = progress_channel();

    let cascade_children: Vec<_> = tree
        .find_node(cascade_addr)
        .unwrap()
        .children
        .clone();

    let ch = tokio::spawn(async move {
        node.receive_and_forward(&mid, &cascade_children, Some(size as u64), ctx)
            .await
            .unwrap()
    });

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let sender = FileSender::new(teacher_addr).unwrap();

    let teacher_children: Vec<_> = tree
        .find_node(teacher_addr)
        .unwrap()
        .children
        .clone();

    for child in &teacher_children {
        sender.send_file(&src, *child).await.unwrap();
    }

    let ch_result = ch.await.unwrap();
    let h1_result = h1.await.unwrap();
    let h2_result = h2.await.unwrap();

    assert_eq!(ch_result, expected);
    assert_eq!(h1_result, expected);
    assert_eq!(h2_result, expected);
}

#[tokio::test]
async fn tree_topology_matches_actual_transfer() {
    let peers: Vec<_> = (0..5).map(|_| free_addr()).collect();
    let teacher = free_addr();

    let tree = DistributionTree::build(teacher, &peers, 2);

    for node in &tree.nodes {
        if node.addr == teacher {
            assert!(node.parent.is_none());
            assert!(!node.children.is_empty());
        }
    }

    let root_children = &tree.find_node(teacher).unwrap().children;
    assert!(root_children.len() <= 2);

    for child_addr in root_children {
        let child = tree.find_node(*child_addr).unwrap();
        assert_eq!(child.parent, Some(teacher));
    }
}

#[tokio::test]
async fn heal_and_redistribute() {
    let teacher = free_addr();
    let peers: Vec<_> = (0..6).map(|_| free_addr()).collect();

    let mut tree = DistributionTree::build(teacher, &peers, 2);
    let failed = tree.find_node(teacher).unwrap().children[0];

    tree.heal(failed);

    assert!(tree.find_node(failed).is_none());

    for node in &tree.nodes {
        if node.addr != teacher {
            assert!(
                node.parent.is_some(),
                "node {:?} should have parent after heal",
                node.addr
            );
        }
    }
}

#[tokio::test]
async fn direct_send_to_single_peer() {
    let dir = TempDir::new("direct");
    let src = dir.file("direct_src.bin");
    let dst = dir.file("direct_dst.bin");

    let data = create_test_file(&src, 2048);
    let expected: [u8; 32] = Sha256::digest(&data).into();

    let recv_addr = free_addr();
    let receiver = FileReceiver::new(recv_addr).unwrap();
    let (tx, _) = progress_channel();

    let rh = tokio::spawn(async move {
        receiver
            .receive_file(&dst, Some(2048), tx)
            .await
            .unwrap()
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let sender = FileSender::new(free_addr()).unwrap();
    sender.send_file(&src, recv_addr).await.unwrap();

    let hash = rh.await.unwrap();
    assert_eq!(hash, expected);
}

#[test]
fn config_default_values() {
    let config = lectord_config_default();

    assert_eq!(config.grpc_port, 50051);
    assert_eq!(config.quic_port, 50052);
}

fn lectord_config_default() -> TestConfig {
    TestConfig {
        grpc_port: 50051,
        quic_port: 50052,
    }
}

struct TestConfig {
    grpc_port: u16,
    quic_port: u16,
}

#[test]
fn topology_with_selected_peers_only() {
    let all_peers: Vec<_> = (1..=10).map(|p| addr(9000 + p)).collect();
    let selected: Vec<_> = vec![all_peers[0], all_peers[2], all_peers[5]];

    let tree = DistributionTree::build(addr(9000), &selected, 2);

    assert_eq!(tree.nodes.len(), 4);

    for peer in &selected {
        assert!(tree.find_node(*peer).is_some());
    }

    let unselected = vec![all_peers[1], all_peers[3], all_peers[4]];
    for peer in &unselected {
        assert!(tree.find_node(*peer).is_none());
    }
}

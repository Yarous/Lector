mod common;

use lector_topology::DistributionTree;
use common::addr;

#[test]
fn build_single_peer() {
    let tree = DistributionTree::build(addr(9000), &[addr(9001)], 2);

    assert_eq!(tree.nodes.len(), 2);
    assert_eq!(tree.root, addr(9000));

    let root = tree.find_node(addr(9000)).unwrap();
    assert_eq!(root.children, vec![addr(9001)]);
    assert!(root.parent.is_none());

    let child = tree.find_node(addr(9001)).unwrap();
    assert!(child.children.is_empty());
    assert_eq!(child.parent, Some(addr(9000)));
}

#[test]
fn build_binary_tree_structure() {
    let peers: Vec<_> = (1..=7).map(|p| addr(9000 + p)).collect();
    let tree = DistributionTree::build(addr(9000), &peers, 2);

    assert_eq!(tree.nodes.len(), 8);

    let root = tree.find_node(addr(9000)).unwrap();
    assert_eq!(root.children.len(), 2);

    let left = tree.find_node(root.children[0]).unwrap();
    let right = tree.find_node(root.children[1]).unwrap();
    assert_eq!(left.children.len(), 2);
    assert_eq!(right.children.len(), 2);
}

#[test]
fn build_ternary_tree() {
    let peers: Vec<_> = (1..=9).map(|p| addr(9000 + p)).collect();
    let tree = DistributionTree::build(addr(9000), &peers, 3);

    let root = tree.find_node(addr(9000)).unwrap();
    assert_eq!(root.children.len(), 3);
}

#[test]
fn build_with_branching_factor_one() {
    let peers: Vec<_> = (1..=4).map(|p| addr(9000 + p)).collect();
    let tree = DistributionTree::build(addr(9000), &peers, 1);

    let root = tree.find_node(addr(9000)).unwrap();
    assert_eq!(root.children.len(), 1);

    let second = tree.find_node(root.children[0]).unwrap();
    assert_eq!(second.children.len(), 1);
}

#[test]
fn build_empty_peers() {
    let tree = DistributionTree::build(addr(9000), &[], 2);

    assert_eq!(tree.nodes.len(), 1);
    let root = tree.find_node(addr(9000)).unwrap();
    assert!(root.children.is_empty());
}

#[test]
fn every_node_except_root_has_parent() {
    let peers: Vec<_> = (1..=15).map(|p| addr(9000 + p)).collect();
    let tree = DistributionTree::build(addr(9000), &peers, 2);

    for node in &tree.nodes {
        if node.addr == addr(9000) {
            assert!(node.parent.is_none());
        } else {
            assert!(node.parent.is_some(), "node {:?} has no parent", node.addr);
        }
    }
}

#[test]
fn all_peers_present_in_tree() {
    let peers: Vec<_> = (1..=20).map(|p| addr(9000 + p)).collect();
    let tree = DistributionTree::build(addr(9000), &peers, 2);

    for peer in &peers {
        assert!(
            tree.find_node(*peer).is_some(),
            "peer {:?} missing from tree",
            peer
        );
    }
}

#[test]
fn parent_child_consistency() {
    let peers: Vec<_> = (1..=10).map(|p| addr(9000 + p)).collect();
    let tree = DistributionTree::build(addr(9000), &peers, 2);

    for node in &tree.nodes {
        for child_addr in &node.children {
            let child = tree.find_node(*child_addr).unwrap();
            assert_eq!(
                child.parent,
                Some(node.addr),
                "child {:?} should have parent {:?}",
                child_addr,
                node.addr
            );
        }
    }
}

#[test]
fn remove_leaf_node() {
    let peers: Vec<_> = (1..=3).map(|p| addr(9000 + p)).collect();
    let mut tree = DistributionTree::build(addr(9000), &peers, 2);

    let orphans = tree.remove_node(addr(9003));

    assert!(orphans.is_empty());
    assert!(tree.find_node(addr(9003)).is_none());
    assert_eq!(tree.nodes.len(), 3);
}

#[test]
fn remove_intermediate_node_returns_orphans() {
    let peers: Vec<_> = (1..=6).map(|p| addr(9000 + p)).collect();
    let mut tree = DistributionTree::build(addr(9000), &peers, 2);

    let node_to_remove = tree.find_node(addr(9000)).unwrap().children[0];
    let expected_orphans = tree.find_node(node_to_remove).unwrap().children.clone();

    let orphans = tree.remove_node(node_to_remove);

    assert_eq!(orphans.len(), expected_orphans.len());
    for orphan in &orphans {
        let node = tree.find_node(*orphan).unwrap();
        assert!(node.parent.is_none());
    }
}

#[test]
fn heal_reconnects_orphans() {
    let peers: Vec<_> = (1..=6).map(|p| addr(9000 + p)).collect();
    let mut tree = DistributionTree::build(addr(9000), &peers, 2);

    let failed = tree.find_node(addr(9000)).unwrap().children[0];
    let orphan_count = tree.find_node(failed).unwrap().children.len();

    tree.heal(failed);

    assert!(tree.find_node(failed).is_none());

    let mut parentless = 0;
    for node in &tree.nodes {
        if node.addr != addr(9000) && node.parent.is_none() {
            parentless += 1;
        }
    }
    assert_eq!(parentless, 0, "all orphans should be reconnected");
}

#[test]
fn heal_preserves_remaining_nodes() {
    let peers: Vec<_> = (1..=10).map(|p| addr(9000 + p)).collect();
    let mut tree = DistributionTree::build(addr(9000), &peers, 2);
    let total_before = tree.nodes.len();

    tree.heal(addr(9001));

    assert_eq!(tree.nodes.len(), total_before - 1);
    assert!(tree.find_node(addr(9000)).is_some());
}

#[test]
fn reattach_updates_both_sides() {
    let peers: Vec<_> = (1..=4).map(|p| addr(9000 + p)).collect();
    let mut tree = DistributionTree::build(addr(9000), &peers, 2);

    tree.reattach(addr(9003), addr(9002));

    let new_parent = tree.find_node(addr(9002)).unwrap();
    assert!(new_parent.children.contains(&addr(9003)));

    let child = tree.find_node(addr(9003)).unwrap();
    assert_eq!(child.parent, Some(addr(9002)));
}

#[test]
fn find_least_loaded_excludes_root() {
    let tree = DistributionTree::build(addr(9000), &[addr(9001)], 2);

    let least = tree.find_least_loaded_node();
    assert_eq!(least, Some(addr(9001)));
}

#[test]
fn heal_multiple_failures() {
    let peers: Vec<_> = (1..=15).map(|p| addr(9000 + p)).collect();
    let mut tree = DistributionTree::build(addr(9000), &peers, 2);

    tree.heal(addr(9001));
    tree.heal(addr(9002));
    tree.heal(addr(9003));

    for node in &tree.nodes {
        if node.addr != addr(9000) {
            assert!(
                node.parent.is_some(),
                "node {:?} orphaned after multiple heals",
                node.addr
            );
        }
    }
}

#[test]
fn large_tree_stress() {
    let peers: Vec<_> = (1..=1000).map(|p| addr(p)).collect();
    let tree = DistributionTree::build(addr(0), &peers, 2);

    assert_eq!(tree.nodes.len(), 1001);

    for node in &tree.nodes {
        assert!(node.children.len() <= 2);
    }
}

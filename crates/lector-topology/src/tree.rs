use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub addr: SocketAddr,
    pub children: Vec<SocketAddr>,
    pub parent: Option<SocketAddr>,
}

#[derive(Debug, Clone)]
pub struct DistributionTree {
    pub root: SocketAddr,
    pub nodes: Vec<TreeNode>,
}

impl DistributionTree {
    pub fn build(root: SocketAddr, peers: &[SocketAddr], branching_factor: usize) -> Self {
        let bf = branching_factor.max(1);
        let mut nodes = vec![TreeNode {
            addr: root,
            children: Vec::new(),
            parent: None,
        }];

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(0usize);
        let mut peer_idx = 0;

        while peer_idx < peers.len() {
            let Some(parent_pos) = queue.pop_front() else { break };

            let assigned: Vec<SocketAddr> = peers[peer_idx..]
                .iter()
                .take(bf)
                .copied()
                .collect();

            let count = assigned.len();
            let parent_addr = nodes[parent_pos].addr;

            for &child_addr in &assigned {
                let child_pos = nodes.len();
                nodes.push(TreeNode {
                    addr: child_addr,
                    children: Vec::new(),
                    parent: Some(parent_addr),
                });
                nodes[parent_pos].children.push(child_addr);
                queue.push_back(child_pos);
            }

            peer_idx += count;
        }

        Self { root, nodes }
    }

    pub fn find_node(&self, addr: SocketAddr) -> Option<&TreeNode> {
        self.nodes.iter().find(|n| n.addr == addr)
    }

    pub fn remove_node(&mut self, failed: SocketAddr) -> Vec<SocketAddr> {
        let orphans: Vec<SocketAddr> = self
            .nodes
            .iter()
            .filter(|n| n.addr == failed)
            .flat_map(|n| n.children.clone())
            .collect();

        self.nodes.retain(|n| n.addr != failed);

        for node in &mut self.nodes {
            node.children.retain(|c| *c != failed);
        }

        for orphan_addr in &orphans {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.addr == *orphan_addr) {
                node.parent = None;
            }
        }

        orphans
    }

    pub fn reattach(&mut self, orphan: SocketAddr, new_parent: SocketAddr) {
        if let Some(parent) = self.nodes.iter_mut().find(|n| n.addr == new_parent) {
            if !parent.children.contains(&orphan) {
                parent.children.push(orphan);
            }
        }
        if let Some(child) = self.nodes.iter_mut().find(|n| n.addr == orphan) {
            child.parent = Some(new_parent);
        }
    }

    pub fn find_least_loaded_node(&self) -> Option<SocketAddr> {
        self.nodes
            .iter()
            .filter(|n| n.addr != self.root)
            .min_by_key(|n| n.children.len())
            .map(|n| n.addr)
    }

    pub fn heal(&mut self, failed: SocketAddr) {
        let orphans = self.remove_node(failed);
        for orphan in orphans {
            let new_parent = self.find_least_loaded_node().unwrap_or(self.root);
            self.reattach(orphan, new_parent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    #[test]
    fn builds_binary_tree() {
        let tree = DistributionTree::build(addr(1000), &[addr(1), addr(2), addr(3), addr(4), addr(5)], 2);
        assert_eq!(tree.nodes.len(), 6);
        let root = tree.find_node(addr(1000)).unwrap();
        assert_eq!(root.children.len(), 2);
    }

    #[test]
    fn heals_after_failure() {
        let mut tree = DistributionTree::build(addr(1000), &[addr(1), addr(2), addr(3), addr(4)], 2);
        tree.heal(addr(1));
        assert!(tree.find_node(addr(1)).is_none());
        assert!(tree.find_node(addr(3)).unwrap().parent.is_some());
    }
}

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct AppState {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug)]
struct Inner {
    peer_addresses: Vec<SocketAddr>,
    selected_file: Option<PathBuf>,
    selected_peers: Vec<bool>,
}

impl AppState {
    pub fn new() -> Self {
        let default_peers: Vec<SocketAddr> = (1..=100)
            .map(|i| format!("192.168.1.{}:50051", i).parse().unwrap())
            .collect();

        let count = default_peers.len();

        Self {
            inner: Arc::new(Mutex::new(Inner {
                peer_addresses: default_peers,
                selected_file: None,
                selected_peers: vec![false; count],
            })),
        }
    }

    pub fn peers(&self) -> Vec<SocketAddr> {
        self.inner.lock().unwrap().peer_addresses.clone()
    }

    pub fn replace_peers(&self, peers: Vec<SocketAddr>) {
        let mut inner = self.inner.lock().unwrap();
        inner.peer_addresses = peers;
        inner.selected_peers = vec![false; inner.peer_addresses.len()];
    }

    pub fn set_selected_file(&self, path: PathBuf) {
        self.inner.lock().unwrap().selected_file = Some(path);
    }

    pub fn selected_file(&self) -> Option<PathBuf> {
        self.inner.lock().unwrap().selected_file.clone()
    }

    pub fn set_peer_selected(&self, idx: usize, selected: bool) {
        let mut inner = self.inner.lock().unwrap();
        if idx < inner.selected_peers.len() {
            inner.selected_peers[idx] = selected;
        }
    }

    pub fn select_all_online(&self, online_indices: &[usize]) {
        let mut inner = self.inner.lock().unwrap();
        for &idx in online_indices {
            if idx < inner.selected_peers.len() {
                inner.selected_peers[idx] = true;
            }
        }
    }

    pub fn deselect_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.selected_peers.iter_mut().for_each(|s| *s = false);
    }

    pub fn selected_peer_addresses(&self) -> Vec<SocketAddr> {
        let inner = self.inner.lock().unwrap();
        inner
            .peer_addresses
            .iter()
            .zip(inner.selected_peers.iter())
            .filter(|(_, selected)| **selected)
            .map(|(addr, _)| *addr)
            .collect()
    }

    pub fn is_selected(&self, idx: usize) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.selected_peers.get(idx).copied().unwrap_or(false)
    }

    pub fn selected_count(&self) -> usize {
        self.inner
            .lock()
            .unwrap()
            .selected_peers
            .iter()
            .filter(|s| **s)
            .count()
    }

    pub fn resize_selection(&self, count: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.selected_peers.resize(count, false);
    }
}

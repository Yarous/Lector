mod app_state;
mod discovery;
mod monitor;
mod orchestrator;

use anyhow::Result;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use tracing_subscriber::EnvFilter;

slint::include_modules!();

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("lector_ui=info".parse()?))
        .init();

    let ui = MainWindow::new()?;
    let state = app_state::AppState::new();
    setup_callbacks(&ui, &state);
    ui.run()?;
    Ok(())
}

fn setup_callbacks(ui: &MainWindow, state: &app_state::AppState) {
    setup_scan(ui, state);
    setup_file_selection(ui, state);
    setup_peer_toggle(ui, state);
    setup_select_all(ui, state);
    setup_deselect_all(ui, state);
    setup_distribution(ui, state);
    setup_counters(ui, state);
}

fn setup_scan(ui: &MainWindow, state: &app_state::AppState) {
    let ui_weak = ui.as_weak();
    let state = state.clone();

    ui.global::<AppBridge>().on_scan_network(move || {
        let ui = ui_weak.clone();
        let state = state.clone();
        let peers = state.peers();

        set_bridge(&ui, |b| {
            b.set_scanning(true);
            b.set_status_text("Scanning network...".into());
        });

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let results = rt.block_on(discovery::scan_network(&peers));
            state.resize_selection(results.len());

            slint::invoke_from_event_loop(move || {
                let Some(ui) = ui.upgrade() else { return };
                let bridge = ui.global::<AppBridge>();

                let model: Vec<PeerInfo> = results.iter().enumerate().map(|(idx, r)| PeerInfo {
                    address: r.addr.to_string().into(),
                    hostname: r.hostname.clone().into(),
                    status: if r.online { "online" } else { "offline" }.into(),
                    ping_ms: r.ping_ms as i32,
                    progress: 0,
                    free_disk_gb: r.free_disk_gb as f32,
                    selected: state.is_selected(idx),
                    version: r.version.clone().into(),
                }).collect();

                let online = results.iter().filter(|r| r.online).count();
                let total = results.len();

                bridge.set_peers(ModelRc::from(Rc::new(VecModel::from(model))));
                bridge.set_scanning(false);
                bridge.set_status_text(
                    format!("Scan complete — {} of {} peers online", online, total).into(),
                );
            }).ok();
        });
    });
}

fn setup_file_selection(ui: &MainWindow, state: &app_state::AppState) {
    let ui_weak = ui.as_weak();
    let state = state.clone();

    ui.global::<AppBridge>().on_select_file(move || {
        let dialog = rfd::FileDialog::new()
            .set_title("Choose a file to distribute")
            .add_filter("All files", &["*"]);

        if let Some(path) = dialog.pick_file() {
            let name = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let size = std::fs::metadata(&path)
                .map(|m| format_size(m.len()))
                .unwrap_or_default();

            state.set_selected_file(path);

            if let Some(ui) = ui_weak.upgrade() {
                let bridge = ui.global::<AppBridge>();
                bridge.set_selected_file_name(name.into());
                bridge.set_selected_file_size(size.into());
                bridge.set_selected_file_path(SharedString::from("selected"));
            }
        }
    });
}

fn setup_peer_toggle(ui: &MainWindow, state: &app_state::AppState) {
    let ui_weak = ui.as_weak();
    let state = state.clone();

    ui.global::<AppBridge>().on_peer_toggled(move |idx, val| {
        state.set_peer_selected(idx as usize, val);
        refresh_peers_selection(&ui_weak, &state);
    });
}

fn setup_select_all(ui: &MainWindow, state: &app_state::AppState) {
    let ui_weak = ui.as_weak();
    let state = state.clone();

    ui.global::<AppBridge>().on_select_all(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let bridge = ui.global::<AppBridge>();
            let peers = bridge.get_peers();
            let online_indices: Vec<usize> = (0..peers.row_count())
                .filter(|i| peers.row_data(*i).map(|p| p.status == "online").unwrap_or(false))
                .collect();

            state.select_all_online(&online_indices);
            refresh_peers_selection(&ui_weak, &state);
        }
    });
}

fn setup_deselect_all(ui: &MainWindow, state: &app_state::AppState) {
    let ui_weak = ui.as_weak();
    let state = state.clone();

    ui.global::<AppBridge>().on_deselect_all(move || {
        state.deselect_all();
        refresh_peers_selection(&ui_weak, &state);
    });
}

fn setup_distribution(ui: &MainWindow, state: &app_state::AppState) {
    let ui_weak = ui.as_weak();
    let state = state.clone();

    ui.global::<AppBridge>().on_start_distribution(move || {
        let ui = ui_weak.clone();
        let state = state.clone();

        set_bridge(&ui, |b| {
            b.set_distributing(true);
            b.set_overall_progress(0);
            b.set_status_text("Starting distribution...".into());
        });

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(orchestrator::distribute(state, ui));
        });
    });

    let ui_weak = ui.as_weak();
    ui.global::<AppBridge>().on_cancel_distribution(move || {
        set_bridge(&ui_weak, |b| {
            b.set_distributing(false);
            b.set_status_text("Distribution cancelled".into());
        });
    });
}

fn setup_counters(ui: &MainWindow, state: &app_state::AppState) {
    let state_sel = state.clone();
    ui.global::<AppBridge>().on_count_selected(move || {
        state_sel.selected_count() as i32
    });

    let ui_weak = ui.as_weak();
    ui.global::<AppBridge>().on_count_online(move || {
        let Some(ui) = ui_weak.upgrade() else { return 0 };
        let peers = ui.global::<AppBridge>().get_peers();
        (0..peers.row_count())
            .filter(|i| peers.row_data(*i).map(|p| p.status == "online").unwrap_or(false))
            .count() as i32
    });
}

fn refresh_peers_selection(ui_weak: &slint::Weak<MainWindow>, state: &app_state::AppState) {
    let Some(ui) = ui_weak.upgrade() else { return };
    let bridge = ui.global::<AppBridge>();
    let peers = bridge.get_peers();

    let updated: Vec<PeerInfo> = (0..peers.row_count())
        .filter_map(|i| peers.row_data(i).map(|mut p| {
            p.selected = state.is_selected(i);
            p
        }))
        .collect();

    bridge.set_peers(ModelRc::from(Rc::new(VecModel::from(updated))));
}

fn set_bridge(ui: &slint::Weak<MainWindow>, f: impl FnOnce(&AppBridge) + Send + 'static) {
    let ui = ui.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            f(&ui.global::<AppBridge>());
        }
    }).ok();
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    match bytes {
        b if b >= GB => format!("{:.1} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{:.1} KB", b as f64 / KB as f64),
        b => format!("{} B", b),
    }
}

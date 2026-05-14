mod hyprland;
mod niri;
mod sway;

use gtk4::prelude::*;
use gtk4::{Box, Button, Orientation};

#[derive(Clone)]
enum WorkspaceTarget {
    HyprlandId(i64),
    NiriId(u64),
    SwayName(String),
}

#[derive(Clone)]
struct WorkspaceButtonData {
    target: WorkspaceTarget,
    name: String,
    focused: bool,
    sort_key: i64,
}

pub fn init(container: &gtk4::Box) {
    let workspaces_box = Box::new(Orientation::Horizontal, 0);
    workspaces_box.add_css_class("workspaces-box");
    container.append(&workspaces_box);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<WorkspaceButtonData>>();
    let (tx_ws, rx_ws) = tokio::sync::mpsc::unbounded_channel::<WorkspaceTarget>();

    let wbox = workspaces_box.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(ws_data) = rx.recv().await {
            update_workspaces(&wbox, ws_data, tx_ws.clone());
        }
    });

    std::thread::spawn(move || match niri::connect_socket() {
        Ok(socket) => niri::run(socket, rx_ws, tx),
        Err(_) if hyprland::is_available() => hyprland::run(rx_ws, tx),
        Err(_) => sway::run(rx_ws, tx),
    });
}

fn get_workspace_icon(name: &str) -> String {
    match name {
        "1" => "".to_string(),
        "2" => "".to_string(),
        "3" => "".to_string(),
        "4" => "".to_string(),
        "5" => "".to_string(),
        "6" => "".to_string(),
        "7" => "".to_string(),
        _ => "".to_string(),
    }
}

fn update_workspaces(
    container: &Box,
    mut ws_data: Vec<WorkspaceButtonData>,
    tx: tokio::sync::mpsc::UnboundedSender<WorkspaceTarget>,
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    ws_data.sort_by_key(|ws| ws.sort_key);

    for ws in ws_data {
        let btn = Button::builder()
            .label(get_workspace_icon(&ws.name))
            .build();

        btn.add_css_class("workspace-btn");
        btn.add_css_class("btn");
        if ws.focused {
            btn.add_css_class("focused");
        }

        let tx_clone = tx.clone();
        let target = ws.target.clone();
        btn.connect_clicked(move |_| {
            let _ = tx_clone.send(target.clone());
        });

        container.append(&btn);
    }
}

use gtk4::prelude::*;
use gtk4::{Box, Button, Orientation};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

struct WorkspaceState {
    workspaces: Vec<(u64, String, bool, u8)>, // (id, display_name, is_focused, idx)
}

impl WorkspaceState {
    fn from_json(val: serde_json::Value) -> Self {
        let workspaces = val
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| {
                let id = w.get("id")?.as_u64()?;
                let idx = w.get("idx").and_then(|i| i.as_u64()).unwrap_or(id) as u8;
                let name = w
                    .get("name")
                    .and_then(|n| n.as_str().map(String::from))
                    .unwrap_or_else(|| idx.to_string());
                let focused = w.get("is_focused").and_then(|f| f.as_bool()).unwrap_or(false);
                Some((id, name, focused, idx))
            })
            .collect();
        Self { workspaces }
    }

    fn handle_workspace_activated(&mut self, id: u64, focused: bool) {
        for (ws_id, _, is_focused, _) in &mut self.workspaces {
            if *ws_id == id {
                *is_focused = focused;
            } else if focused {
                *is_focused = false;
            }
        }
    }

    fn to_ui_data(&self) -> Vec<(u64, String, bool)> {
        let mut sorted = self.workspaces.clone();
        sorted.sort_by_key(|(_, _, _, idx)| *idx);
        sorted
            .into_iter()
            .map(|(id, name, focused, _)| (id, name, focused))
            .collect()
    }
}

fn send_request(stream: &mut UnixStream, request: &str) -> Result<serde_json::Value, String> {
    stream
        .write_all(format!("{}\n", request).as_bytes())
        .map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;

    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&line).map_err(|e| e.to_string())
}

fn connect_socket() -> Result<UnixStream, String> {
    let path = std::env::var("NIRI_SOCKET").map_err(|e| e.to_string())?;
    UnixStream::connect(&path).map_err(|e| e.to_string())
}

pub fn init(container: &gtk4::Box) {
    let workspaces_box = Box::new(Orientation::Horizontal, 0);
    workspaces_box.add_css_class("workspaces-box");
    container.append(&workspaces_box);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<(u64, String, bool)>>();
    let (tx_ws, mut rx_ws) = tokio::sync::mpsc::unbounded_channel::<u64>();

    let wbox = workspaces_box.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(ws_data) = rx.recv().await {
            update_workspaces(&wbox, ws_data, tx_ws.clone());
        }
    });

    std::thread::spawn(move || {
        let socket = match connect_socket() {
            Ok(s) => s,
            Err(_) => {
                eprintln!("[workspaces] niri socket not available");
                return;
            }
        };

        let action_socket = match connect_socket() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[workspaces] niri action socket failed: {}", e);
                return;
            }
        };

        let mut action_stream = action_socket;
        std::thread::spawn(move || {
            while let Some(ws_id) = rx_ws.blocking_recv() {
                let action = serde_json::json!({
                    "Action": {
                        "FocusWorkspace": {
                            "reference": { "Id": ws_id }
                        }
                    }
                });
                let _ = send_request(&mut action_stream, &action.to_string());
            }
        });

        let event_result = send_request(
            &mut socket.try_clone().unwrap(),
            &serde_json::json!("EventStream").to_string(),
        );

        match event_result {
            Ok(resp) => eprintln!("[workspaces] EventStream response: {:?}", resp),
            Err(e) => {
                eprintln!("[workspaces] EventStream failed: {}", e);
                return;
            }
        }

        let _ = socket.shutdown(std::net::Shutdown::Write);
        let mut reader = BufReader::new(socket);
        let mut line = String::new();
        let mut state = WorkspaceState {
            workspaces: Vec::new(),
        };

        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            let event: serde_json::Value = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("[workspaces] parse error: {}", e);
                    continue;
                }
            };

            if let Some(ws) = event.get("WorkspacesChanged") {
                state = WorkspaceState::from_json(ws["workspaces"].clone());
                let _ = tx.send(state.to_ui_data());
            } else if let Some(activated) = event.get("WorkspaceActivated") {
                if let (Some(id), Some(focused)) = (
                    activated.get("id").and_then(|v| v.as_u64()),
                    activated.get("focused").and_then(|v| v.as_bool()),
                ) {
                    state.handle_workspace_activated(id, focused);
                    let _ = tx.send(state.to_ui_data());
                }
            } else if event.get("WorkspaceUrgencyChanged").is_some()
                || event.get("WindowsChanged").is_some()
            {
                let _ = tx.send(state.to_ui_data());
            }
        }
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
    ws_data: Vec<(u64, String, bool)>,
    tx: tokio::sync::mpsc::UnboundedSender<u64>,
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    for (ws_id, name, is_focused) in ws_data {
        let btn = Button::builder().label(get_workspace_icon(&name)).build();

        if is_focused {
            btn.add_css_class("workspace-btn");
            btn.add_css_class("btn");
            btn.add_css_class("focused");
        } else {
            btn.add_css_class("workspace-btn");
            btn.add_css_class("btn");
        }

        let tx_clone = tx.clone();
        btn.connect_clicked(move |_| {
            let _ = tx_clone.send(ws_id);
        });

        container.append(&btn);
    }
}

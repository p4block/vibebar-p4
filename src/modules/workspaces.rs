use gtk4::prelude::*;
use gtk4::{Button, Box, Orientation};
use tokio::runtime::Runtime;
use futures::StreamExt;

pub fn init(container: &gtk4::Box) {
    let workspaces_box = Box::new(Orientation::Horizontal, 0);
    workspaces_box.set_widget_name("workspaces");
    container.append(&workspaces_box);

    let _workspaces_box_clone = workspaces_box.clone();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<(String, bool)>>();
    
    let wbox = workspaces_box.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(ws_data) = rx.recv().await {
            update_workspaces(&wbox, ws_data);
        }
    });

    std::thread::spawn(move || {
        // Try Niri first
        if let Ok(mut socket) = niri_ipc::socket::Socket::connect() {
            if let Ok(Ok(niri_ipc::Response::Handled)) = socket.send(niri_ipc::Request::EventStream) {
                let mut read_event = socket.read_events();
                while let Ok(event) = read_event() {
                    match event {
                        niri_ipc::Event::WorkspacesChanged { workspaces } => {
                            let ws_data: Vec<(String, bool)> = workspaces.into_iter()
                                .map(|w| (w.name.unwrap_or_else(|| w.id.to_string()), w.is_active))
                                .collect();
                            let _ = tx.send(ws_data);
                        }
                        _ => {}
                    }
                }
            }
        } else {
            // Fallback to Sway
            let rt = Runtime::new().unwrap();
            rt.block_on(async move {
                if let Ok(sway_for_events) = swayipc_async::Connection::new().await {
                    let mut events = sway_for_events.subscribe([swayipc_async::EventType::Workspace]).await.unwrap();
                    
                    let mut sway_for_queries = swayipc_async::Connection::new().await.unwrap();

                    // Initial fetch
                    if let Ok(ws) = sway_for_queries.get_workspaces().await {
                        let ws_data = ws.into_iter().map(|w| (w.name, w.focused)).collect();
                        let _ = tx.send(ws_data);
                    }

                    while let Some(Ok(event)) = events.next().await {
                       if let swayipc_async::Event::Workspace(_) = event {
                           if let Ok(ws) = sway_for_queries.get_workspaces().await {
                               let ws_data = ws.into_iter().map(|w| (w.name, w.focused)).collect();
                               let _ = tx.send(ws_data);
                           }
                       }
                    }
                }
            });
        }
    });
}

fn roman_numeral(name: &str) -> String {
    match name {
        "1" => "I".to_string(),
        "2" => "II".to_string(),
        "3" => "III".to_string(),
        "4" => "IV".to_string(),
        "5" => "V".to_string(),
        "6" => "VI".to_string(),
        "7" => "VII".to_string(),
        "8" => "VIII".to_string(),
        "9" => "IX".to_string(),
        "10" => "X".to_string(),
        _ => name.to_string(),
    }
}

fn update_workspaces(container: &Box, ws_data: Vec<(String, bool)>) {
    // Clear existing
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    for (name, is_focused) in ws_data {
        let label_text = roman_numeral(&name);
        let btn = Button::builder()
            .label(&label_text)
            .build();
        
        btn.add_css_class("workspace-btn");
        if is_focused {
            btn.add_css_class("focused");
            btn.add_css_class("active");
        }
        
        container.append(&btn);
    }
}

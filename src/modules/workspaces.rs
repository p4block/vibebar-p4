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
    
    let (tx_ws, mut rx_ws) = tokio::sync::mpsc::unbounded_channel::<String>();

    let wbox = workspaces_box.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(ws_data) = rx.recv().await {
            update_workspaces(&wbox, ws_data, tx_ws.clone());
        }
    });

    std::thread::spawn(move || {
        // Try Niri first
        if let Ok(mut socket) = niri_ipc::socket::Socket::connect() {
            let mut socket_clone = niri_ipc::socket::Socket::connect().unwrap();
            std::thread::spawn(move || {
                while let Some(name) = rx_ws.blocking_recv() {
                    let _ = socket_clone.send(niri_ipc::Request::Action(niri_ipc::Action::FocusWorkspace { 
                        reference: niri_ipc::WorkspaceReferenceArg::Name(name) 
                    }));
                }
            });

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
                    let mut sway_for_commands = swayipc_async::Connection::new().await.unwrap();

                    tokio::spawn(async move {
                        while let Some(name) = rx_ws.recv().await {
                            let _ = sway_for_commands.run_command(format!("workspace \"{}\"", name)).await;
                        }
                    });

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

fn update_workspaces(container: &Box, ws_data: Vec<(String, bool)>, tx: tokio::sync::mpsc::UnboundedSender<String>) {
    // Clear existing
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    for (name, is_focused) in ws_data {
        let label_text = get_workspace_icon(&name);
        let btn = Button::builder()
            .label(&label_text)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();
        
        if let Some(child) = btn.child() {
            child.set_halign(gtk4::Align::Center);
            child.set_valign(gtk4::Align::Center);
        }
        
        btn.add_css_class("workspace-btn");
        if is_focused {
            btn.add_css_class("focused");
            btn.add_css_class("active");
        }

        let tx_clone = tx.clone();
        let name_clone = name.clone();
        btn.connect_clicked(move |_| {
            let _ = tx_clone.send(name_clone.clone());
        });
        
        container.append(&btn);
    }
}

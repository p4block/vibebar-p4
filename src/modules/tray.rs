use gtk4::prelude::*;
use gtk4::{Box, Orientation, Image, Button};
use system_tray::client::Client;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub fn init(container: &gtk4::Box) {
    let tray_box = Box::new(Orientation::Horizontal, 0);
    tray_box.set_widget_name("tray");
    container.append(&tray_box);

    let items: Arc<Mutex<HashMap<String, gtk4::Widget>>> = Arc::new(Mutex::new(HashMap::new()));

    let (tx_ui, mut rx_ui) = tokio::sync::mpsc::unbounded_channel::<(String, Option<String>)>();
    let (tx_activate, mut rx_activate) = tokio::sync::mpsc::unbounded_channel::<String>();

    let tbox = tray_box.clone();
    let titems = items.clone();
    let tx_act = tx_activate.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some((id, icon_name)) = rx_ui.recv().await {
            let mut map = titems.lock().unwrap();
            if let Some(name) = icon_name {
                if !map.contains_key(&id) {
                    let img = Image::builder()
                        .icon_name(&name)
                        .pixel_size(18)
                        .build();
                    img.set_css_classes(&["tray-icon"]);
                    
                    let btn = Button::builder()
                        .child(&img)
                        .build();
                    btn.set_widget_name("tray-btn");
                    
                    let id_clone = id.clone();
                    let tx_active_clone = tx_act.clone();
                    btn.connect_clicked(move |_| {
                        let _ = tx_active_clone.send(id_clone.clone());
                    });

                    tbox.append(&btn);
                    map.insert(id, btn.upcast());
                } else if let Some(widget) = map.get(&id) {
                    if let Some(btn) = widget.downcast_ref::<Button>() {
                        if let Some(img) = btn.child().and_then(|c| c.downcast::<Image>().ok()) {
                            img.set_icon_name(Some(&name));
                        }
                    }
                }
            } else {
                if let Some(widget) = map.remove(&id) {
                    tbox.remove(&widget);
                }
            }
        }
    });

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Ok(client) = Client::new().await {
                let client = Arc::new(client);
                let mut stream = client.subscribe();
                
                let client_activate = client.clone();
                tokio::spawn(async move {
                    while let Some(id) = rx_activate.recv().await {
                        let _ = client_activate.activate(system_tray::client::ActivateRequest::Default {
                            address: id,
                            x: 0,
                            y: 0,
                        }).await;
                    }
                });

                while let Ok(event) = stream.recv().await {
                    match event {
                        system_tray::client::Event::Add(id, item) => {
                            let icon = item.icon_name.clone().unwrap_or_else(|| "image-missing".to_string());
                            let _ = tx_ui.send((id, Some(icon)));
                        }
                        system_tray::client::Event::Update(id, update) => {
                            if let system_tray::client::UpdateEvent::Icon { icon_name, .. } = update {
                                if let Some(name) = icon_name {
                                    let _ = tx_ui.send((id, Some(name)));
                                }
                            }
                        }
                        system_tray::client::Event::Remove(id) => {
                            let _ = tx_ui.send((id, None));
                        }
                    }
                }
            }
        });
    });
}

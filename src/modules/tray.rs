use gtk4::prelude::*;
use gtk4::{Box, Button, GestureClick, Image, Orientation, Popover};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use system_tray::client::{ActivateRequest, Client, UpdateEvent};
use system_tray::menu::TrayMenu;
use tokio::sync::{broadcast, mpsc};

#[derive(Clone, Debug)]
pub struct TrayItemInfo {
    pub icon_name: Option<String>,
    pub icon_theme_path: Option<String>,
    pub icon_pixmap: Option<Vec<system_tray::item::IconPixmap>>,
    pub status: system_tray::item::Status,
}

enum TrayAction {
    Activate {
        id: String,
        x: i32,
        y: i32,
    },
    MenuAction {
        id: String,
        menu_path: String,
        item_id: i32,
    },
}

struct MenuState {
    menu_path: String,
    menu: Option<TrayMenu>,
}

pub struct TrayBackend {
    _client: Arc<Client>,
    tx_activate: mpsc::UnboundedSender<TrayAction>,
    titems: Arc<Mutex<HashMap<String, TrayItemInfo>>>,
    tmenus: Arc<Mutex<HashMap<String, MenuState>>>,
    tx_ui: broadcast::Sender<(String, Option<TrayItemInfo>)>,
}

impl TrayBackend {
    pub async fn new() -> Option<Arc<Self>> {
        let client = Client::new().await.ok()?;
        let mut stream = client.subscribe();
        let client = Arc::new(client);
        let (tx_activate, mut rx_activate) = mpsc::unbounded_channel::<TrayAction>();
        let (tx_ui, _) = broadcast::channel(128);

        let titems = Arc::new(Mutex::new(HashMap::new()));
        let tmenus = Arc::new(Mutex::new(HashMap::new()));

        {
            let client_items = client.items();
            let lock = client_items.lock().unwrap();
            let mut items = titems.lock().unwrap();
            let mut menus = tmenus.lock().unwrap();
            for (id, (item, menu)) in lock.iter() {
                let info = TrayItemInfo {
                    icon_name: item.icon_name.clone(),
                    icon_theme_path: item.icon_theme_path.clone(),
                    icon_pixmap: item.icon_pixmap.clone(),
                    status: item.status,
                };
                items.insert(id.clone(), info);
                if let Some(menu_path) = &item.menu {
                    menus.insert(
                        id.clone(),
                        MenuState {
                            menu_path: menu_path.clone(),
                            menu: menu.clone(),
                        },
                    );
                }
            }
        }

        let titems_thread = titems.clone();
        let tmenus_thread = tmenus.clone();
        let tx_ui_thread = tx_ui.clone();
        let client_act = client.clone();

        tokio::spawn(async move {
            tokio::spawn(async move {
                while let Some(action) = rx_activate.recv().await {
                    match action {
                        TrayAction::Activate { id, x, y } => {
                            let _ = client_act
                                .activate(ActivateRequest::Default { address: id, x, y })
                                .await;
                        }
                        TrayAction::MenuAction {
                            id,
                            menu_path,
                            item_id,
                        } => {
                            let _ = client_act
                                .activate(ActivateRequest::MenuItem {
                                    address: id,
                                    menu_path,
                                    submenu_id: item_id,
                                })
                                .await;
                        }
                    }
                }
            });

            while let Ok(event) = stream.recv().await {
                match event {
                    system_tray::client::Event::Add(id, item) => {
                        let info = TrayItemInfo {
                            icon_name: item.icon_name.clone(),
                            icon_theme_path: item.icon_theme_path.clone(),
                            icon_pixmap: item.icon_pixmap.clone(),
                            status: item.status,
                        };
                        {
                            let mut items = titems_thread.lock().unwrap();
                            items.insert(id.clone(), info.clone());
                        }
                        if let Some(menu_path) = item.menu {
                            let mut m = tmenus_thread.lock().unwrap();
                            m.insert(
                                id.clone(),
                                MenuState {
                                    menu_path,
                                    menu: None,
                                },
                            );
                        }
                        let _ = tx_ui_thread.send((id, Some(info)));
                    }
                    system_tray::client::Event::Update(id, update) => match update {
                        UpdateEvent::Status(status) => {
                            let mut items = titems_thread.lock().unwrap();
                            if let Some(info) = items.get_mut(&id) {
                                info.status = status;
                                let info_clone = info.clone();
                                drop(items);
                                let _ = tx_ui_thread.send((id, Some(info_clone)));
                            }
                        }
                        UpdateEvent::Icon {
                            icon_name,
                            icon_pixmap,
                            ..
                        } => {
                            let mut items = titems_thread.lock().unwrap();
                            if let Some(info) = items.get_mut(&id) {
                                if let Some(name) = icon_name {
                                    info.icon_name = Some(name);
                                }
                                if let Some(pix) = icon_pixmap {
                                    info.icon_pixmap = Some(pix);
                                }
                                let info_clone = info.clone();
                                drop(items);
                                let _ = tx_ui_thread.send((id, Some(info_clone)));
                            }
                        }
                        UpdateEvent::Menu(menu) => {
                            let mut m = tmenus_thread.lock().unwrap();
                            if let Some(ms) = m.get_mut(&id) {
                                ms.menu = Some(menu);
                            }
                        }
                        UpdateEvent::MenuConnect(menu_path) => {
                            let mut m = tmenus_thread.lock().unwrap();
                            if let Some(ms) = m.get_mut(&id) {
                                ms.menu_path = menu_path;
                            } else {
                                m.insert(
                                    id,
                                    MenuState {
                                        menu_path,
                                        menu: None,
                                    },
                                );
                            }
                        }
                        _ => {}
                    },
                    system_tray::client::Event::Remove(id) => {
                        titems_thread.lock().unwrap().remove(&id);
                        tmenus_thread.lock().unwrap().remove(&id);
                        let _ = tx_ui_thread.send((id, None));
                    }
                }
            }
        });

        Some(Arc::new(Self {
            _client: client,
            tx_activate,
            titems,
            tmenus,
            tx_ui,
        }))
    }
}

pub fn init(container: &gtk4::Box, backend: Arc<TrayBackend>) {
    let tray_box = Box::new(Orientation::Horizontal, 0);
    tray_box.set_widget_name("tray");
    container.append(&tray_box);

    let gui_items: Arc<Mutex<HashMap<String, gtk4::Widget>>> = Arc::new(Mutex::new(HashMap::new()));

    let tbox = tray_box.clone();
    let tgui_items = gui_items.clone();
    let backend_ui = backend.clone();
    let mut rx_ui_global = backend.tx_ui.subscribe();

    gtk4::glib::MainContext::default().spawn_local(async move {
        // Initial Sync Logic
        {
            let items = backend_ui.titems.lock().unwrap();
            let mut map = tgui_items.lock().unwrap();
            for (id, info) in items.iter() {
                let btn = Button::builder()
                    .css_classes(vec!["btn".to_string()])
                    .build();
                let is_visible = matches!(
                    info.status,
                    system_tray::item::Status::Active | system_tray::item::Status::NeedsAttention
                );
                btn.set_visible(is_visible);
                update_icon(&btn, info);
                setup_button_signals(&btn, id, &backend_ui);
                tbox.append(&btn);
                map.insert(id.clone(), btn.upcast());
            }
        }

        loop {
            match rx_ui_global.recv().await {
                Ok((id, info_opt)) => {
                    let mut map = tgui_items.lock().unwrap();
                    if let Some(info) = info_opt {
                        let is_visible = matches!(
                            info.status,
                            system_tray::item::Status::Active
                                | system_tray::item::Status::NeedsAttention
                        );

                        if let Some(widget) = map.get(&id) {
                            if let Some(btn) = widget.downcast_ref::<Button>() {
                                btn.set_visible(is_visible);
                                update_icon(btn, &info);
                            }
                        } else {
                            let btn = Button::builder()
                                .css_classes(vec!["btn".to_string()])
                                .build();
                            btn.set_visible(is_visible);
                            update_icon(&btn, &info);
                            setup_button_signals(&btn, &id, &backend_ui);
                            tbox.append(&btn);
                            map.insert(id, btn.upcast());
                        }
                    } else {
                        if let Some(widget) = map.remove(&id) {
                            tbox.remove(&widget);
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Full reconciliation on lag
                    let items = backend_ui.titems.lock().unwrap();
                    let mut map = tgui_items.lock().unwrap();

                    map.retain(|id, widget| {
                        if !items.contains_key(id) {
                            tbox.remove(widget);
                            false
                        } else {
                            true
                        }
                    });

                    for (id, info) in items.iter() {
                        let is_visible = matches!(
                            info.status,
                            system_tray::item::Status::Active
                                | system_tray::item::Status::NeedsAttention
                        );
                        if let Some(widget) = map.get(id) {
                            if let Some(btn) = widget.downcast_ref::<Button>() {
                                btn.set_visible(is_visible);
                                update_icon(btn, info);
                            }
                        } else {
                            let btn = Button::builder()
                                .css_classes(vec!["btn".to_string()])
                                .build();
                            btn.set_visible(is_visible);
                            update_icon(&btn, info);
                            setup_button_signals(&btn, id, &backend_ui);
                            tbox.append(&btn);
                            map.insert(id.clone(), btn.upcast());
                        }
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn setup_button_signals(btn: &Button, id: &str, backend: &Arc<TrayBackend>) {
    let id_clone = id.to_string();
    let tx_act = backend.tx_activate.clone();
    let btn_handle = btn.clone();

    btn.connect_clicked(move |_| {
        let mut x_abs = 0;
        let mut y_abs = 0;
        if let Some(window) = btn_handle
            .root()
            .and_then(|r| r.downcast::<gtk4::Window>().ok())
        {
            let (x, y) = btn_handle
                .translate_coordinates(&window, 0.0, 0.0)
                .unwrap_or((0.0, 0.0));

            if let Some(surface) = window.surface() {
                // EXPLICITLY specify the trait (WidgetExt vs RootExt)
                if let Some(monitor) =
                    gtk4::prelude::WidgetExt::display(&window).monitor_at_surface(&surface)
                {
                    let geom = monitor.geometry();
                    x_abs = geom.x() + x as i32;
                    y_abs = geom.y() + y as i32;
                }
            }
        }
        let _ = tx_act.send(TrayAction::Activate {
            id: id_clone.clone(),
            x: x_abs,
            y: y_abs,
        });
    });

    let gesture = GestureClick::new();
    gesture.set_button(3);
    let id_right = id.to_string();
    let tx_act_right = backend.tx_activate.clone();
    let btn_right = btn.clone();
    let backend_right = backend.clone();

    gesture.connect_pressed(move |_, _, _, _| {
        let menu_opt = {
            let m = backend_right.tmenus.lock().unwrap();
            m.get(&id_right)
                .and_then(|ms| ms.menu.clone().map(|menu| (ms.menu_path.clone(), menu)))
        };

        if let Some((menu_path, menu)) = menu_opt {
            let popover = Popover::builder()
                .position(gtk4::PositionType::Top)
                .autohide(true)
                .has_arrow(true)
                .build();
            popover.set_parent(&btn_right);

            let vbox = create_menu_vbox(
                menu.submenus,
                id_right.clone(),
                menu_path,
                tx_act_right.clone(),
                Some(popover.clone()),
            );
            popover.set_child(Some(&vbox));
            popover.popup();
        }
    });
    btn.add_controller(gesture);
}

fn update_icon(btn: &Button, info: &TrayItemInfo) {
    let theme = gtk4::IconTheme::for_display(&gtk4::gdk::Display::default().unwrap());
    if let Some(path) = &info.icon_theme_path {
        if !theme.search_path().iter().any(|p| p.to_str() == Some(path)) {
            theme.add_search_path(path);
        }
    }

    if let Some(name) = &info.icon_name {
        let img = Image::builder().icon_name(name).pixel_size(18).build();
        btn.set_child(Some(&img));
    } else if let Some(pixmaps) = &info.icon_pixmap {
        if let Some(pixmap) = pixmaps.iter().max_by_key(|p| p.width) {
            let mut rgba_pixels = Vec::with_capacity(pixmap.pixels.len());
            for chunk in pixmap.pixels.chunks_exact(4) {
                // ARGB to RGBA
                rgba_pixels.push(chunk[1]);
                rgba_pixels.push(chunk[2]);
                rgba_pixels.push(chunk[3]);
                rgba_pixels.push(chunk[0]);
            }
            let bytes = gtk4::glib::Bytes::from(&rgba_pixels);
            let pixbuf = gdk4::gdk_pixbuf::Pixbuf::from_bytes(
                &bytes,
                gdk4::gdk_pixbuf::Colorspace::Rgb,
                true,
                8,
                pixmap.width,
                pixmap.height,
                pixmap.width * 4,
            );
            let img = Image::builder()
                .paintable(&gtk4::gdk::Texture::for_pixbuf(&pixbuf))
                .pixel_size(18)
                .build();
            btn.set_child(Some(&img));
        }
    } else {
        btn.set_child(Some(&Image::from_icon_name("image-missing")));
    }
}

fn create_menu_vbox(
    items: Vec<system_tray::menu::MenuItem>,
    id: String,
    menu_path: String,
    tx_activate: mpsc::UnboundedSender<TrayAction>,
    parent_popover: Option<Popover>,
) -> Box {
    let vbox = Box::new(Orientation::Vertical, 0);
    for item in items {
        if !item.visible {
            continue;
        }
        if item.menu_type == system_tray::menu::MenuType::Separator {
            vbox.append(&gtk4::Separator::new(Orientation::Horizontal));
            continue;
        }

        let item_box = Box::new(Orientation::Horizontal, 5);
        let label = gtk4::Label::new(item.label.as_deref());
        label.set_halign(gtk4::Align::Start);
        label.set_hexpand(true);
        item_box.append(&label);

        if !item.submenu.is_empty() {
            item_box.append(&Image::from_icon_name("pan-end-symbolic"));
        }

        let item_btn = Button::builder().child(&item_box).build();
        item_btn.set_sensitive(item.enabled);

        if !item.submenu.is_empty() {
            let id_sub = id.clone();
            let path_sub = menu_path.clone();
            let sub_items = item.submenu.clone();
            let tx_sub = tx_activate.clone();
            let btn_sub = item_btn.clone();

            item_btn.connect_clicked(move |_| {
                let sub_pop = Popover::builder()
                    .position(gtk4::PositionType::Right)
                    .autohide(true)
                    .build();
                sub_pop.set_parent(&btn_sub);
                let sub_vbox = create_menu_vbox(
                    sub_items.clone(),
                    id_sub.clone(),
                    path_sub.clone(),
                    tx_sub.clone(),
                    Some(sub_pop.clone()),
                );
                sub_pop.set_child(Some(&sub_vbox));
                sub_pop.popup();
            });
        } else {
            let id_act = id.clone();
            let path_act = menu_path.clone();
            let item_id = item.id;
            let tx_act_clone = tx_activate.clone();
            let p_pop = parent_popover.clone();
            item_btn.connect_clicked(move |_| {
                let _ = tx_act_clone.send(TrayAction::MenuAction {
                    id: id_act.clone(),
                    menu_path: path_act.clone(),
                    item_id,
                });
                if let Some(p) = &p_pop {
                    p.popdown();
                }
            });
        }
        vbox.append(&item_btn);
    }
    vbox
}

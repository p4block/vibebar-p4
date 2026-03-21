use std::collections::HashMap;
use std::time::Duration;
use tokio::time;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::composite::ConnectionExt as _;
use x11rb::protocol::xproto::{ConnectionExt as _, *};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
#[derive(Clone, Debug)]
pub enum XEmbedEvent {
    Add(Window),
    Remove(Window),
    Update(Window, Vec<u8>), // window_id, rgba_pixels
}

pub struct XEmbedBackend {
    conn: Arc<RustConnection>,
    _screen_num: usize,
    _root: Window,
    manager_win: Window,
    items: Arc<Mutex<HashMap<Window, Vec<u8>>>>,
    tx_ui: broadcast::Sender<XEmbedEvent>,
    tx_action: mpsc::UnboundedSender<XEmbedAction>,
}

pub enum XEmbedAction {
    Click { window_id: Window, x: i32, y: i32, button: u32 },
    Dismiss,
}

impl XEmbedBackend {
    pub async fn new() -> Option<Arc<Self>> {
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let conn = Arc::new(conn);
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        let win_id = conn.generate_id().ok()?;
        
        // Find a 32-bit visual if possible for true transparency
        let mut visual_id = screen.root_visual;
        let mut depth = screen.root_depth;
        for d in &screen.allowed_depths {
            if d.depth == 32 {
                if let Some(v) = d.visuals.first() {
                    visual_id = v.visual_id;
                    depth = 32;
                    break;
                }
            }
        }

        let colormap = if depth == 32 {
            let cid = conn.generate_id().ok()?;
            let _ = conn.create_colormap(ColormapAlloc::NONE, cid, root, visual_id);
            Some(cid)
        } else {
            None
        };

        let mut aux = CreateWindowAux::new()
            .event_mask(EventMask::STRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT)
            .override_redirect(1)
            .background_pixel(0);
        
        if let Some(cm) = colormap {
            aux = aux.colormap(cm).border_pixel(0);
        }

        conn.create_window(
            depth,
            win_id,
            root,
            -32, -32, 24, 24, // 24x24 and off-screen
            0,
            WindowClass::INPUT_OUTPUT,
            visual_id,
            &aux,
        ).ok()?;
        conn.map_window(win_id).ok()?;

        let selection_name = format!("_NET_SYSTEM_TRAY_S{}", screen_num);
        let selection_atom = conn.intern_atom(false, selection_name.as_bytes()).ok()?.reply().ok()?.atom;
        let manager_atom = conn.intern_atom(false, b"MANAGER").ok()?.reply().ok()?.atom;
        let opcode_atom = conn.intern_atom(false, b"_NET_SYSTEM_TRAY_OPCODE").ok()?.reply().ok()?.atom;
        let orientation_atom = conn.intern_atom(false, b"_NET_SYSTEM_TRAY_ORIENTATION").ok()?.reply().ok()?.atom;
        let visual_atom = conn.intern_atom(false, b"_NET_SYSTEM_TRAY_VISUAL").ok()?.reply().ok()?.atom;
        let opacity_atom = conn.intern_atom(false, b"_NET_WM_WINDOW_OPACITY").ok()?.reply().ok()?.atom;

        // Try to initialize composite extension
        let composite_present = conn.extension_information(x11rb::protocol::composite::X11_EXTENSION_NAME).ok().flatten().is_some();
        if composite_present {
            let _ = x11rb::protocol::composite::query_version(&*conn, 0, 4);
        }

        // Set ownership
        conn.set_selection_owner(win_id, selection_atom, x11rb::CURRENT_TIME).ok()?;
        let owner = conn.get_selection_owner(selection_atom).ok()?.reply().ok()?.owner;
        if owner != win_id {
            return None;
        }

        // Set properties
        let _ = conn.change_property32(
            PropMode::REPLACE,
            win_id,
            orientation_atom,
            AtomEnum::CARDINAL,
            &[0], // SYSTEM_TRAY_ORIENTATION_HORIZ
        );
        let _ = conn.change_property32(
            PropMode::REPLACE,
            win_id,
            visual_atom,
            AtomEnum::VISUALID,
            &[screen.root_visual],
        );
        let _ = conn.change_property32(
            PropMode::REPLACE,
            win_id,
            opacity_atom,
            AtomEnum::CARDINAL,
            &[0], // 0% opacity
        );
        
        // Send Manager event
        let event = ClientMessageEvent::new(
            32,
            root,
            manager_atom,
            [x11rb::CURRENT_TIME, selection_atom, win_id, 0, 0]
        );
        conn.send_event(false, root, EventMask::STRUCTURE_NOTIFY, event).ok()?;
        let _ = conn.flush();

        let items = Arc::new(Mutex::new(HashMap::new()));
        let (tx_ui, _) = broadcast::channel(64);
        let (tx_action, mut rx_action) = mpsc::unbounded_channel();

        let backend = Arc::new(Self {
            conn: conn.clone(),
            _screen_num: screen_num,
            _root: root,
            manager_win: win_id,
            items: items.clone(),
            tx_ui,
            tx_action,
        });

        let backend_clone = backend.clone();
        tokio::spawn(async move {
            let conn = backend_clone.conn.clone();
            loop {
                if let Ok(event) = conn.wait_for_event() {
                    match event {
                        Event::ClientMessage(ev) => {
                            if ev.type_ == opcode_atom {
                                let data = ev.data.as_data32();
                                if data[1] == 0 { // SYSTEM_TRAY_REQUEST_DOCK
                                    let docked_win = data[2];
                                    let timestamp = data[0];
                                    if docked_win != 0 {
                                        backend_clone.dock_window(docked_win, timestamp).await;
                                    }
                                } else {
                                    // println!("[XEmbed] Unknown opcode: {}", data[1]);
                                }
                            }
                        },
                        Event::DestroyNotify(ev) => {
                            backend_clone.undock_window(ev.window).await;
                        }
                        _ => {}
                    }
                }
            }
        });

        // Loop for capturing and actions
        let backend_capture = backend.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        backend_capture.refresh_icons().await;
                    }
                    Some(action) = rx_action.recv() => {
                        backend_capture.handle_action(action).await;
                    }
                }
            }
        });

        Some(backend)
    }

    async fn dock_window(&self, win: Window, timestamp: u32) {
        // Atoms for XEmbed
        let xembed_atom = self.conn.intern_atom(false, b"_XEMBED").ok().and_then(|c| c.reply().ok()).map(|r| r.atom).unwrap_or(0);
        
        // Redirect the window so it renders even when hidden
        let _ = self.conn.composite_redirect_window(win, x11rb::protocol::composite::Redirect::AUTOMATIC);
        
        // Prepare the window for embedding
        let _ = self.conn.change_window_attributes(win, &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE));
        let _ = self.conn.configure_window(win, &ConfigureWindowAux::new().width(24).height(24));
        let _ = self.conn.reparent_window(win, self.manager_win, 0, 0);
        let _ = self.conn.map_window(win);
        
        // XEmbed EMBEDDED_NOTIFY
        if xembed_atom != 0 {
            let event = ClientMessageEvent::new(
                32,
                win,
                xembed_atom,
                [timestamp, 0, 0, self.manager_win, 0] // 0=time, 1=opcode (XEMBED_EMBEDDED_NOTIFY=0), 2=version, 3=manager_win
            );
            let _ = self.conn.send_event(false, win, EventMask::NO_EVENT, event);

            // XEmbed WINDOW_ACTIVATE
            let activate = ClientMessageEvent::new(
                32, win, xembed_atom,
                [timestamp, 1, 0, 0, 0] // 1 = XEMBED_WINDOW_ACTIVATE
            );
            let _ = self.conn.send_event(false, win, EventMask::NO_EVENT, activate);

            // XEmbed FOCUS_IN
            let focus = ClientMessageEvent::new(
                32, win, xembed_atom,
                [timestamp, 4, 0, 0, 0] // 4 = XEMBED_FOCUS_IN
            );
            let _ = self.conn.send_event(false, win, EventMask::NO_EVENT, focus);
        }
        
        let _ = self.conn.flush();

        let mut items = self.items.lock().unwrap();
        items.insert(win, Vec::new());
        let _ = self.tx_ui.send(XEmbedEvent::Add(win));
    }

    async fn undock_window(&self, win: Window) {
        let mut items = self.items.lock().unwrap();
        if items.remove(&win).is_some() {
            let _ = self.tx_ui.send(XEmbedEvent::Remove(win));
        }
    }

    async fn refresh_icons(&self) {
        let win_ids: Vec<Window> = {
            let items = self.items.lock().unwrap();
            items.keys().cloned().collect()
        };

        static FAIL_COUNT: AtomicU32 = AtomicU32::new(0);

        for win in win_ids {
            // Check if window is still mapped/valid
            let attr = self.conn.get_window_attributes(win).ok().and_then(|c| c.reply().ok());
            if attr.as_ref().map(|a| a.map_state) != Some(MapState::VIEWABLE) {
                continue;
            }

            if let Ok(geom) = self.conn.get_geometry(win).map_err(|_| ()).and_then(|c| c.reply().map_err(|_| ())) {
                if let Ok(img) = self.conn.get_image(ImageFormat::Z_PIXMAP, win, 0, 0, geom.width, geom.height, 0xffffffff).map_err(|_| ()).and_then(|c| c.reply().map_err(|_| ())) {
                    let rgba = bgra_to_rgba(&img.data, geom.width as usize, geom.height as usize);
                    let mut items = self.items.lock().unwrap();
                    let updated = if let Some(old) = items.get(&win) {
                        old.len() != rgba.len() || old != &rgba
                    } else {
                        true
                    };
                    if updated {
                        items.insert(win, rgba.clone());
                        let _ = self.tx_ui.send(XEmbedEvent::Update(win, rgba));
                        // println!("[XEmbed] Sent update for 0x{:x}", win);
                    }
                } else {
                    let count = FAIL_COUNT.fetch_add(1, Ordering::Relaxed);
                    if count % 100 == 0 {
                        // Throttled logging for capture failure
                    }
                }
            }
        }
    }

    async fn handle_action(&self, action: XEmbedAction) {
        match action {
            XEmbedAction::Click { window_id, x, y, button } => {
                // Get current global pointer position
                if let Some(reply) = self.conn.query_pointer(self._root).ok().and_then(|c| c.reply().ok()) {
                    let root_x = reply.root_x;
                    let root_y = reply.root_y;

                    // Move the manager window to the mouse position
                    let win_x = (root_x - x as i16) as i32;
                    let win_y = (root_y - y as i16) as i32;
                    // println!("[XEmbed] Moving manager to {}, {} (mouse: {}, {})", win_x, win_y, root_x, root_y);
                    
                    let _ = self.conn.configure_window(self.manager_win, &ConfigureWindowAux::new()
                        .x(win_x)
                        .y(win_y));
                    let _ = self.conn.flush();
                    
                    // Ultra-short delay to let XWayland/Wine notice the move
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                    // Send ButtonPress
                    let press = ButtonPressEvent {
                        response_type: BUTTON_PRESS_EVENT,
                        detail: button as u8,
                        sequence: 0,
                        time: x11rb::CURRENT_TIME,
                        root: self._root,
                        event: window_id,
                        child: 0,
                        root_x,
                        root_y,
                        event_x: x as i16,
                        event_y: y as i16,
                        state: KeyButMask::default(),
                        same_screen: true,
                    };
                    let _ = self.conn.send_event(true, window_id, EventMask::NO_EVENT, press);

                    // Send ButtonRelease
                    let release = ButtonReleaseEvent {
                        response_type: BUTTON_RELEASE_EVENT,
                        detail: button as u8,
                        sequence: 0,
                        time: x11rb::CURRENT_TIME,
                        root: self._root,
                        event: window_id,
                        child: 0,
                        root_x,
                        root_y,
                        event_x: x as i16,
                        event_y: y as i16,
                        state: KeyButMask::default(),
                        same_screen: true,
                    };
                    let _ = self.conn.send_event(true, window_id, EventMask::NO_EVENT, release);
                    let _ = self.conn.flush();

                    // Return immediately
                    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
                    let _ = self.conn.configure_window(self.manager_win, &ConfigureWindowAux::new()
                        .x(-100)
                        .y(-100));
                    let _ = self.conn.flush();
                }
            }
            XEmbedAction::Dismiss => {
                // Aggressively break any existing grabs and steal focus.
                // Teleport to (0,0) briefly to ensure the window is "viewable" by X11 standards.
                let _ = self.conn.configure_window(self.manager_win, &ConfigureWindowAux::new().x(0).y(0));
                let _ = self.conn.grab_pointer(
                    false,
                    self.manager_win,
                    EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                    x11rb::NONE,
                    x11rb::NONE,
                    x11rb::CURRENT_TIME,
                );
                let _ = self.conn.ungrab_pointer(x11rb::CURRENT_TIME);
                let _ = self.conn.set_input_focus(InputFocus::PARENT, self.manager_win, x11rb::CURRENT_TIME);
                
                // Return to hiding
                let _ = self.conn.configure_window(self.manager_win, &ConfigureWindowAux::new().x(-100).y(-100));
                let _ = self.conn.flush();
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<XEmbedEvent> {
        self.tx_ui.subscribe()
    }

    pub fn send_click(&self, window_id: Window, x: i32, y: i32, button: u32) {
        let _ = self.tx_action.send(XEmbedAction::Click { window_id, x, y, button });
    }

    pub fn dismiss_menus(&self) {
        let _ = self.tx_action.send(XEmbedAction::Dismiss);
    }
}

#[allow(dead_code)]
async fn selection_name_atom(conn: Arc<RustConnection>, screen: usize) -> Option<Atom> {
    let name = format!("_NET_SYSTEM_TRAY_S{}", screen);
    conn.intern_atom(false, name.as_bytes()).ok()?.reply().ok().map(|r| r.atom)
}

fn bgra_to_rgba(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(width * height * 4);
    for chunk in data.chunks_exact(4) {
        // X11 ZPixmap is usually BGRA or BGRx
        rgba.push(chunk[2]); // R
        rgba.push(chunk[1]); // G
        rgba.push(chunk[0]); // B
        rgba.push(chunk[3]); // A (Preserve alpha from source)
    }
    rgba
}

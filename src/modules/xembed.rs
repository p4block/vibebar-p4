use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::time;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::Event;
use x11rb::protocol::composite::ConnectionExt as _;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{ConnectionExt as _, *};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

const ICON_SIZE: u16 = 24;
const HIDDEN_X: i32 = -32_000;
const HIDDEN_Y: i32 = -32_000;
const MAX_FAILS: u32 = 3;

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
    avoid_onscreen_manager: bool,
    items: Arc<Mutex<HashMap<Window, Vec<u8>>>>,
    fail_counts: Arc<Mutex<HashMap<Window, u32>>>,
    tx_ui: broadcast::Sender<XEmbedEvent>,
    tx_action: mpsc::UnboundedSender<XEmbedAction>,
}

pub enum XEmbedAction {
    Click {
        window_id: Window,
        x: i32,
        y: i32,
        root_x: Option<i32>,
        root_y: Option<i32>,
        button: u32,
    },
    Dismiss,
}

impl XEmbedBackend {
    pub async fn new() -> Option<Arc<Self>> {
        if std::env::var("VIBEBAR_NO_XEMBED").is_ok() {
            return None;
        }
        let avoid_onscreen_manager = should_avoid_onscreen_manager();
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
            HIDDEN_X as i16,
            HIDDEN_Y as i16,
            ICON_SIZE,
            ICON_SIZE,
            0,
            WindowClass::INPUT_OUTPUT,
            visual_id,
            &aux,
        )
        .ok()?;

        let selection_name = format!("_NET_SYSTEM_TRAY_S{}", screen_num);
        let selection_atom = conn
            .intern_atom(false, selection_name.as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom;
        let manager_atom = conn.intern_atom(false, b"MANAGER").ok()?.reply().ok()?.atom;
        let opcode_atom = conn
            .intern_atom(false, b"_NET_SYSTEM_TRAY_OPCODE")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let orientation_atom = conn
            .intern_atom(false, b"_NET_SYSTEM_TRAY_ORIENTATION")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let visual_atom = conn
            .intern_atom(false, b"_NET_SYSTEM_TRAY_VISUAL")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let opacity_atom = conn
            .intern_atom(false, b"_NET_WM_WINDOW_OPACITY")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let wm_name_atom = conn.intern_atom(false, b"WM_NAME").ok()?.reply().ok()?.atom;
        let wm_class_atom = conn
            .intern_atom(false, b"WM_CLASS")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let net_wm_name_atom = conn
            .intern_atom(false, b"_NET_WM_NAME")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let utf8_string_atom = conn
            .intern_atom(false, b"UTF8_STRING")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let window_type_atom = conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let window_type_dock_atom = conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE_DOCK")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let state_atom = conn
            .intern_atom(false, b"_NET_WM_STATE")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let skip_taskbar_atom = conn
            .intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let skip_pager_atom = conn
            .intern_atom(false, b"_NET_WM_STATE_SKIP_PAGER")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let below_atom = conn
            .intern_atom(false, b"_NET_WM_STATE_BELOW")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let pid_atom = conn
            .intern_atom(false, b"_NET_WM_PID")
            .ok()?
            .reply()
            .ok()?
            .atom;

        // Try to initialize composite extension
        let composite_present = conn
            .extension_information(x11rb::protocol::composite::X11_EXTENSION_NAME)
            .ok()
            .flatten()
            .is_some();
        if composite_present {
            let _ = x11rb::protocol::composite::query_version(&*conn, 0, 4);
        }
        let shape_present = conn
            .extension_information(shape::X11_EXTENSION_NAME)
            .ok()
            .flatten()
            .is_some();
        if shape_present {
            let _ = conn.shape_query_version();
            apply_empty_shape(&conn, win_id);
        }

        // Set ownership
        conn.set_selection_owner(win_id, selection_atom, x11rb::CURRENT_TIME)
            .ok()?;
        let owner = conn
            .get_selection_owner(selection_atom)
            .ok()?
            .reply()
            .ok()?
            .owner;
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
            &[visual_id],
        );
        let _ = conn.change_property32(
            PropMode::REPLACE,
            win_id,
            opacity_atom,
            AtomEnum::CARDINAL,
            &[0], // 0% opacity
        );
        let _ = conn.change_property8(
            PropMode::REPLACE,
            win_id,
            wm_name_atom,
            AtomEnum::STRING,
            b"vibebar-p4-xembed-tray",
        );
        let _ = conn.change_property8(
            PropMode::REPLACE,
            win_id,
            wm_class_atom,
            AtomEnum::STRING,
            b"vibebar-p4-xembed-tray\0vibebar-p4\0",
        );
        let _ = conn.change_property8(
            PropMode::REPLACE,
            win_id,
            net_wm_name_atom,
            utf8_string_atom,
            b"vibebar-p4 XEmbed tray",
        );
        let _ = conn.change_property32(
            PropMode::REPLACE,
            win_id,
            window_type_atom,
            AtomEnum::ATOM,
            &[window_type_dock_atom],
        );
        let _ = conn.change_property32(
            PropMode::REPLACE,
            win_id,
            state_atom,
            AtomEnum::ATOM,
            &[skip_taskbar_atom, skip_pager_atom, below_atom],
        );
        let _ = conn.change_property32(
            PropMode::REPLACE,
            win_id,
            pid_atom,
            AtomEnum::CARDINAL,
            &[std::process::id()],
        );

        // Send Manager event
        let event = ClientMessageEvent::new(
            32,
            root,
            manager_atom,
            [x11rb::CURRENT_TIME, selection_atom, win_id, 0, 0],
        );
        conn.send_event(false, root, EventMask::STRUCTURE_NOTIFY, event)
            .ok()?;
        conn.map_window(win_id).ok()?;
        let _ = conn.flush();

        let items = Arc::new(Mutex::new(HashMap::new()));
        let fail_counts = Arc::new(Mutex::new(HashMap::new()));
        let (tx_ui, _) = broadcast::channel(64);
        let (tx_action, mut rx_action) = mpsc::unbounded_channel();

        let backend = Arc::new(Self {
            conn: conn.clone(),
            _screen_num: screen_num,
            _root: root,
            manager_win: win_id,
            avoid_onscreen_manager,
            items: items.clone(),
            fail_counts: fail_counts.clone(),
            tx_ui,
            tx_action,
        });

        let backend_clone = backend.clone();
        let manager_win = win_id;
        tokio::spawn(async move {
            let conn = backend_clone.conn.clone();
            loop {
                match conn.wait_for_event() {
                    Ok(Event::ClientMessage(ev)) => {
                        if ev.type_ == opcode_atom {
                            let data = ev.data.as_data32();
                            if data[1] == 0 {
                                // SYSTEM_TRAY_REQUEST_DOCK
                                let docked_win = data[2];
                                let timestamp = data[0];
                                if docked_win != 0 {
                                    backend_clone.dock_window(docked_win, timestamp).await;
                                }
                            }
                        }
                    }
                    Ok(Event::DestroyNotify(ev)) => {
                        if ev.window == manager_win {
                            // Manager window was destroyed — clean up all docked windows
                            let mut items = backend_clone.items.lock().unwrap();
                            let windows: Vec<Window> = items.keys().cloned().collect();
                            for w in windows {
                                let _ = backend_clone.tx_ui.send(XEmbedEvent::Remove(w));
                            }
                            items.clear();
                            backend_clone.fail_counts.lock().unwrap().clear();
                            break;
                        }
                        backend_clone.undock_window(ev.window).await;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        // Connection error — wait a bit and retry to avoid tight loop
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        let _ = e;
                    }
                }
            }
            eprintln!("[XEmbed] Event loop exited (manager window destroyed)");
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
        let xembed_atom = self
            .conn
            .intern_atom(false, b"_XEMBED")
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.atom)
            .unwrap_or(0);

        // Redirect the window so it renders even when hidden
        let _ = self
            .conn
            .composite_redirect_window(win, x11rb::protocol::composite::Redirect::AUTOMATIC);

        // Prepare the window for embedding
        let _ = self.conn.change_window_attributes(
            win,
            &ChangeWindowAttributesAux::new().event_mask(
                EventMask::STRUCTURE_NOTIFY
                    | EventMask::SUBSTRUCTURE_NOTIFY
                    | EventMask::PROPERTY_CHANGE,
            ),
        );
        let _ = self
            .conn
            .configure_window(win, &ConfigureWindowAux::new().width(24).height(24));
        let _ = self.conn.reparent_window(win, self.manager_win, 0, 0);
        let _ = self.conn.map_window(win);

        // XEmbed EMBEDDED_NOTIFY
        if xembed_atom != 0 {
            let event = ClientMessageEvent::new(
                32,
                win,
                xembed_atom,
                [timestamp, 0, 0, self.manager_win, 0], // 0=time, 1=opcode (XEMBED_EMBEDDED_NOTIFY=0), 2=version, 3=manager_win
            );
            let _ = self.conn.send_event(false, win, EventMask::NO_EVENT, event);

            // XEmbed WINDOW_ACTIVATE
            let activate = ClientMessageEvent::new(
                32,
                win,
                xembed_atom,
                [timestamp, 1, 0, 0, 0], // 1 = XEMBED_WINDOW_ACTIVATE
            );
            let _ = self
                .conn
                .send_event(false, win, EventMask::NO_EVENT, activate);

            // XEmbed FOCUS_IN
            let focus = ClientMessageEvent::new(
                32,
                win,
                xembed_atom,
                [timestamp, 4, 0, 0, 0], // 4 = XEMBED_FOCUS_IN
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
            let mut fc = self.fail_counts.lock().unwrap();
            fc.remove(&win);
            let _ = self.tx_ui.send(XEmbedEvent::Remove(win));
        }
    }

    async fn refresh_icons(&self) {
        let win_ids: Vec<Window> = {
            let items = self.items.lock().unwrap();
            items.keys().cloned().collect()
        };

        for win in win_ids {
            let attr = match self
                .conn
                .get_window_attributes(win)
                .ok()
                .and_then(|c| c.reply().ok())
            {
                Some(a) => a,
                None => {
                    self.remove_failed_window(win).await;
                    continue;
                }
            };

            if attr.map_state != MapState::VIEWABLE {
                self.remove_failed_window(win).await;
                continue;
            }

            let geom = match self
                .conn
                .get_geometry(win)
                .ok()
                .and_then(|c| c.reply().ok())
            {
                Some(g) => g,
                None => {
                    self.remove_failed_window(win).await;
                    continue;
                }
            };

            if geom.width == 0 || geom.height == 0 {
                continue;
            }

            let img = match self
                .conn
                .get_image(
                    ImageFormat::Z_PIXMAP,
                    win,
                    0,
                    0,
                    geom.width,
                    geom.height,
                    0xffffffff,
                )
                .ok()
                .and_then(|c| c.reply().ok())
            {
                Some(i) => i,
                None => {
                    let should_remove = {
                        let mut fc = self.fail_counts.lock().unwrap();
                        let count = fc.entry(win).or_insert(0);
                        *count += 1;
                        *count >= MAX_FAILS
                    };
                    if should_remove {
                        self.remove_failed_window(win).await;
                    }
                    continue;
                }
            };

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
            }

            let mut fc = self.fail_counts.lock().unwrap();
            if let Some(count) = fc.get_mut(&win) {
                *count = 0;
            }
        }
    }

    async fn remove_failed_window(&self, win: Window) {
        {
            let mut fc = self.fail_counts.lock().unwrap();
            fc.remove(&win);
        }
        let mut items = self.items.lock().unwrap();
        if items.remove(&win).is_some() {
            let _ = self.tx_ui.send(XEmbedEvent::Remove(win));
        }
    }

    async fn handle_action(&self, action: XEmbedAction) {
        match action {
            XEmbedAction::Click {
                window_id,
                x,
                y,
                root_x,
                root_y,
                button,
            } => {
                let fallback_pointer = || {
                    self.conn
                        .query_pointer(self._root)
                        .ok()
                        .and_then(|c| c.reply().ok())
                        .map(|reply| (reply.root_x as i32, reply.root_y as i32))
                };
                let Some((root_x, root_y)) = root_x.zip(root_y).or_else(fallback_pointer) else {
                    return;
                };
                {
                    if button == 3 && self.avoid_onscreen_manager {
                        return;
                    }

                    let move_manager_for_click = button == 3 || !self.avoid_onscreen_manager;

                    // Some XEmbed clients derive menu placement from their actual X11
                    // window position, so context menus still need the short teleport.
                    let win_x = root_x - x;
                    let win_y = root_y - y;
                    // println!("[XEmbed] Moving manager to {}, {} (mouse: {}, {})", win_x, win_y, root_x, root_y);

                    if move_manager_for_click {
                        self.show_click_proxy();
                        let _ = self.conn.configure_window(
                            self.manager_win,
                            &ConfigureWindowAux::new().x(win_x).y(win_y),
                        );
                        let _ = self.conn.flush();

                        // Ultra-short delay to let XWayland/Wine notice the move.
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }

                    // Send ButtonPress
                    let press = ButtonPressEvent {
                        response_type: BUTTON_PRESS_EVENT,
                        detail: button as u8,
                        sequence: 0,
                        time: x11rb::CURRENT_TIME,
                        root: self._root,
                        event: window_id,
                        child: 0,
                        root_x: clamp_i16(root_x),
                        root_y: clamp_i16(root_y),
                        event_x: x as i16,
                        event_y: y as i16,
                        state: KeyButMask::default(),
                        same_screen: true,
                    };
                    let _ = self
                        .conn
                        .send_event(true, window_id, EventMask::NO_EVENT, press);

                    // Send ButtonRelease
                    let release = ButtonReleaseEvent {
                        response_type: BUTTON_RELEASE_EVENT,
                        detail: button as u8,
                        sequence: 0,
                        time: x11rb::CURRENT_TIME,
                        root: self._root,
                        event: window_id,
                        child: 0,
                        root_x: clamp_i16(root_x),
                        root_y: clamp_i16(root_y),
                        event_x: x as i16,
                        event_y: y as i16,
                        state: KeyButMask::default(),
                        same_screen: true,
                    };
                    let _ = self
                        .conn
                        .send_event(true, window_id, EventMask::NO_EVENT, release);
                    let _ = self.conn.flush();

                    // Return immediately
                    if move_manager_for_click {
                        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
                        self.hide_manager_window();
                    }
                }
            }
            XEmbedAction::Dismiss => {
                if !self.avoid_onscreen_manager {
                    // Aggressively break any existing grabs and steal focus.
                    // Teleport briefly to ensure the window is viewable by X11 standards.
                    let _ = self
                        .conn
                        .configure_window(self.manager_win, &ConfigureWindowAux::new().x(0).y(0));
                }
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
                let _ = self.conn.set_input_focus(
                    InputFocus::PARENT,
                    self.manager_win,
                    x11rb::CURRENT_TIME,
                );

                // Return to hiding
                self.hide_manager_window();
            }
        }
    }

    fn hide_manager_window(&self) {
        let _ = self.conn.configure_window(
            self.manager_win,
            &ConfigureWindowAux::new().x(HIDDEN_X).y(HIDDEN_Y),
        );
        apply_empty_shape(&self.conn, self.manager_win);
        let _ = self.conn.flush();
    }

    fn show_click_proxy(&self) {
        apply_icon_shape(&self.conn, self.manager_win);
        let _ = self.conn.configure_window(
            self.manager_win,
            &ConfigureWindowAux::new()
                .width(u32::from(ICON_SIZE))
                .height(u32::from(ICON_SIZE))
                .stack_mode(StackMode::ABOVE),
        );
        let _ = self.conn.map_window(self.manager_win);
        let _ = self.conn.flush();
    }

    pub fn subscribe(&self) -> broadcast::Receiver<XEmbedEvent> {
        self.tx_ui.subscribe()
    }

    pub fn send_click(
        &self,
        window_id: Window,
        x: i32,
        y: i32,
        root_x: Option<i32>,
        root_y: Option<i32>,
        button: u32,
    ) {
        let _ = self.tx_action.send(XEmbedAction::Click {
            window_id,
            x,
            y,
            root_x,
            root_y,
            button,
        });
    }

    pub fn dismiss_menus(&self) {
        let _ = self.tx_action.send(XEmbedAction::Dismiss);
    }
}

fn apply_empty_shape(conn: &RustConnection, win: Window) {
    let empty: [Rectangle; 0] = [];
    let _ = conn.shape_rectangles(
        shape::SO::SET,
        shape::SK::BOUNDING,
        ClipOrdering::UNSORTED,
        win,
        0,
        0,
        &empty,
    );
    let _ = conn.shape_rectangles(
        shape::SO::SET,
        shape::SK::INPUT,
        ClipOrdering::UNSORTED,
        win,
        0,
        0,
        &empty,
    );
}

fn apply_icon_shape(conn: &RustConnection, win: Window) {
    let rect = [Rectangle {
        x: 0,
        y: 0,
        width: ICON_SIZE,
        height: ICON_SIZE,
    }];
    let _ = conn.shape_rectangles(
        shape::SO::SET,
        shape::SK::BOUNDING,
        ClipOrdering::UNSORTED,
        win,
        0,
        0,
        &rect,
    );
    let _ = conn.shape_rectangles(
        shape::SO::SET,
        shape::SK::INPUT,
        ClipOrdering::UNSORTED,
        win,
        0,
        0,
        &rect,
    );
}

fn should_avoid_onscreen_manager() -> bool {
    if std::env::var("VIBEBAR_XEMBED_MOVE_MANAGER").is_ok() {
        return false;
    }
    if std::env::var("VIBEBAR_XEMBED_NEVER_MOVE_MANAGER").is_ok() {
        return true;
    }

    std::env::var("NIRI_SOCKET").is_ok()
        || std::env::var("XDG_CURRENT_DESKTOP")
            .map(|desktop| desktop.to_ascii_lowercase().contains("niri"))
            .unwrap_or(false)
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

#[allow(dead_code)]
async fn selection_name_atom(conn: Arc<RustConnection>, screen: usize) -> Option<Atom> {
    let name = format!("_NET_SYSTEM_TRAY_S{}", screen);
    conn.intern_atom(false, name.as_bytes())
        .ok()?
        .reply()
        .ok()
        .map(|r| r.atom)
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

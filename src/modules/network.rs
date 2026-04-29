use gtk4::prelude::*;
use gtk4::{Box, Button, EventControllerMotion, GestureClick, Label, Orientation, Popover};
use std::cell::Cell;
use std::fs;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct NetworkInfo {
    pub interface: String,
    pub ssid: Option<String>,
    pub conn_type: String,
    pub ip_cidr: String,
    pub strength: Option<u32>,
    pub frequency: Option<u32>,
    pub up_speed: u64,
    pub down_speed: u64,
}

pub fn init(container: &gtk4::Box) {
    let module_box = Box::new(Orientation::Horizontal, 0);
    module_box.set_widget_name("network-module");
    container.append(&module_box);

    let label = Label::builder().label(" ...").build();
    let btn = Button::new();
    btn.add_css_class("btn");
    btn.set_child(Some(&label));
    module_box.append(&btn);

    let popover = Popover::builder()
        .css_classes(vec!["standard-popover".to_string()])
        .position(gtk4::PositionType::Bottom)
        .autohide(false)
        .has_arrow(true)
        .build();
    popover.set_parent(&btn);

    let popover_vbox = Box::new(Orientation::Vertical, 4);
    popover.set_child(Some(&popover_vbox));

    let pop_title = Label::builder().halign(gtk4::Align::Start).build();
    let pop_ip = Label::builder().halign(gtk4::Align::Start).build();
    let pop_wifi = Label::builder().halign(gtk4::Align::Start).build();
    let pop_stats = Label::builder().halign(gtk4::Align::Start).build();

    pop_title.add_css_class("popover-title");
    pop_ip.add_css_class("popover-info");
    pop_wifi.add_css_class("popover-info");
    pop_stats.add_css_class("popover-stats");

    popover_vbox.append(&pop_title);
    popover_vbox.append(&pop_ip);
    popover_vbox.append(&pop_wifi);
    popover_vbox.append(&pop_stats);

    // Click gesture for nmtui
    let click_gesture = GestureClick::new();
    click_gesture.set_button(1); // Left click
    click_gesture.connect_pressed(move |_, _, _, _| {
        let _ = Command::new("footclient").arg("nmtui").spawn();
    });
    btn.add_controller(click_gesture);

    // Hover gesture for popover. Track both widgets because GTK may emit leave
    // on the button when the popover maps or resizes under the pointer.
    let over_button = Rc::new(Cell::new(false));
    let over_popover = Rc::new(Cell::new(false));

    let motion_controller = EventControllerMotion::new();
    let p_enter = popover.clone();
    let over_button_enter = over_button.clone();
    motion_controller.connect_enter(move |_, _, _| {
        over_button_enter.set(true);
        p_enter.popup();
    });

    let p_leave = popover.clone();
    let over_button_leave = over_button.clone();
    let over_popover_leave = over_popover.clone();
    motion_controller.connect_leave(move |_| {
        over_button_leave.set(false);
        schedule_popdown(
            p_leave.clone(),
            over_button_leave.clone(),
            over_popover_leave.clone(),
        );
    });
    btn.add_controller(motion_controller);

    let popover_motion = EventControllerMotion::new();
    let p_popover_enter = popover.clone();
    let over_popover_enter = over_popover.clone();
    popover_motion.connect_enter(move |_, _, _| {
        over_popover_enter.set(true);
        p_popover_enter.popup();
    });

    let p_popover_leave = popover.clone();
    let over_button_popover_leave = over_button.clone();
    let over_popover_leave = over_popover.clone();
    popover_motion.connect_leave(move |_| {
        over_popover_leave.set(false);
        schedule_popdown(
            p_popover_leave.clone(),
            over_button_popover_leave.clone(),
            over_popover_leave.clone(),
        );
    });
    popover.add_controller(popover_motion);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, NetworkInfo)>();

    let label_clone = label.clone();
    let popover_clone = popover.clone();
    let p_title = pop_title.clone();
    let p_ip = pop_ip.clone();
    let p_wifi = pop_wifi.clone();
    let p_stats = pop_stats.clone();

    gtk4::glib::MainContext::default().spawn_local(async move {
        let mut last_display_text = String::new();
        while let Some((display_text, info)) = rx.recv().await {
            if display_text != last_display_text {
                label_clone.set_label(&display_text);
                last_display_text = display_text;
            }

            if popover_clone.is_visible() {
                p_title.set_markup(&format!(
                    "<b>{} @ {}</b>",
                    info.ssid.as_deref().unwrap_or("Internet"),
                    info.conn_type
                ));
                p_ip.set_text(&format!("IP: {}", info.ip_cidr));
                p_stats.set_text(&format!(
                    "Down: {:>5}bps   Up: {:>5}bps",
                    format_speed(info.down_speed),
                    format_speed(info.up_speed)
                ));

                if let (Some(s), Some(f)) = (info.strength, info.frequency) {
                    p_wifi.set_visible(true);
                    p_wifi.set_text(&format!("Strength: {}%  Freq: {}MHz", s, f));
                } else {
                    p_wifi.set_visible(false);
                }
            }
        }
    });

    std::thread::spawn(move || {
        let mut last_rx = 0u64;
        let mut last_tx = 0u64;
        let mut last_iface = String::new();
        let mut last_external_check = 0u32;
        let mut current_ip = "0.0.0.0/0".to_string();
        let mut current_ssid: Option<String> = None;

        loop {
            let mut info = NetworkInfo {
                interface: "none".to_string(),
                ssid: None,
                conn_type: "Disconnected".to_string(),
                ip_cidr: current_ip.clone(),
                strength: None,
                frequency: None,
                up_speed: 0,
                down_speed: 0,
            };

            if let Ok(route_content) = fs::read_to_string("/proc/net/route") {
                let mut best_iface = None;
                let mut min_metric = u32::MAX;

                for line in route_content.lines().skip(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() > 6 && (parts[1] == "00000000" || parts[2] == "00000000") {
                        let iface = parts[0].to_string();
                        if is_virtual_interface(&iface) {
                            continue;
                        }
                        let metric = parts[6].parse::<u32>().unwrap_or(u32::MAX);
                        if metric < min_metric {
                            min_metric = metric;
                            best_iface = Some(iface);
                        }
                    }
                }

                if let Some(iface) = best_iface {
                    // Reset bits counters if interface changed
                    if iface != last_iface {
                        last_rx = 0;
                        last_tx = 0;
                        last_iface = iface.clone();
                        // Force info update on change
                        last_external_check = 0;
                    }

                    info.interface = iface.clone();
                    let is_wifi = fs::metadata(format!("/sys/class/net/{}/wireless", iface))
                        .is_ok()
                        || fs::metadata(format!("/sys/class/net/{}/phy80211", iface)).is_ok();

                    info.conn_type = if is_wifi {
                        "WiFi".to_string()
                    } else {
                        "Ethernet".to_string()
                    };

                    if is_wifi && last_external_check == 0 {
                        if let Ok(output) = Command::new("iwgetid").arg("-r").output() {
                            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            current_ssid = if !s.is_empty() { Some(s) } else { None };
                        }

                        // Signal Strength & Frequency
                        if let Ok(wireless) = fs::read_to_string("/proc/net/wireless") {
                            for line in wireless.lines().skip(2) {
                                if line.contains(&iface) {
                                    let parts: Vec<&str> = line.split_whitespace().collect();
                                    if let Some(lvl) = parts.get(3) {
                                        let level =
                                            lvl.trim_end_matches('.').parse::<f32>().unwrap_or(0.0);
                                        let strength =
                                            ((level + 100.0) * 2.0).clamp(0.0, 100.0) as u32;
                                        info.strength = Some(strength);
                                    }
                                }
                            }
                        }

                        // Frequency using iwconfig
                        if let Ok(output) = Command::new("iwconfig").arg(&iface).output() {
                            let s = String::from_utf8_lossy(&output.stdout);
                            if let Some(pos) = s.find("Frequency:") {
                                let sub = &s[pos + 10..];
                                let freq_str = sub.split_whitespace().next().unwrap_or("0");
                                info.frequency =
                                    Some((freq_str.parse::<f32>().unwrap_or(0.0) * 1000.0) as u32);
                            }
                        }
                    }

                    info.ssid = current_ssid.clone();

                    if last_external_check == 0
                        && let Ok(output) = Command::new("ip")
                            .arg("-4")
                            .arg("addr")
                            .arg("show")
                            .arg(&iface)
                            .output()
                    {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        for line in stdout.lines() {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if let Some(pos) = parts.iter().position(|&r| r == "inet")
                                && let Some(addr) = parts.get(pos + 1)
                            {
                                current_ip = addr.to_string();
                                break;
                            }
                        }
                    }
                    info.ip_cidr = current_ip.clone();

                    if let Some((current_rx, current_tx)) = read_interface_bytes(&iface) {
                        if last_rx > 0 {
                            info.down_speed = current_rx.saturating_sub(last_rx) * 8;
                            info.up_speed = current_tx.saturating_sub(last_tx) * 8;
                        }

                        last_rx = current_rx;
                        last_tx = current_tx;
                    }
                } else {
                    last_rx = 0;
                    last_tx = 0;
                    last_iface.clear();
                    current_ip = "0.0.0.0/0".to_string();
                    current_ssid = None;
                }
            }

            // Sync icon and text
            let icon = if info.conn_type == "WiFi" {
                ""
            } else {
                ""
            };
            let display_text = if info.interface == "none" {
                " Disconnected".to_string()
            } else {
                let ssid_part = info
                    .ssid
                    .as_deref()
                    .map(|s| format!("{} ", s))
                    .unwrap_or_default();
                format!(
                    "{} {}{}",
                    icon,
                    ssid_part,
                    info.ip_cidr.split('/').next().unwrap_or("0.0.0.0")
                )
            };

            let _ = tx.send((display_text, info));

            last_external_check = (last_external_check + 1) % 60; // Check SSID/IP every 120s
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

fn schedule_popdown(popover: Popover, over_button: Rc<Cell<bool>>, over_popover: Rc<Cell<bool>>) {
    gtk4::glib::timeout_add_local_once(Duration::from_millis(250), move || {
        if !over_button.get() && !over_popover.get() {
            popover.popdown();
        }
    });
}

fn read_interface_bytes(iface: &str) -> Option<(u64, u64)> {
    let dev = fs::read_to_string("/proc/net/dev").ok()?;
    for line in dev.lines().skip(2) {
        let (name, stats) = line.split_once(':')?;
        if name.trim() != iface {
            continue;
        }

        let mut parts = stats.split_whitespace();
        let rx = parts.next()?.parse::<u64>().ok()?;
        let tx = parts.nth(7)?.parse::<u64>().ok()?;
        return Some((rx, tx));
    }

    None
}

fn is_virtual_interface(iface: &str) -> bool {
    let virtual_prefixes = [
        "veth", "docker", "br-", "virbr", "cni", "lxc", "tun0", "tap",
    ];
    let virtual_names = ["lo", "docker0", "virbr0"];

    if virtual_names.contains(&iface) {
        return true;
    }

    for prefix in &virtual_prefixes {
        if iface.starts_with(prefix) {
            return true;
        }
    }

    if let Ok(device_type) = fs::read_to_string(format!("/sys/class/net/{}/type", iface))
        && device_type.trim() == "3"
    {
        return true;
    }

    false
}

fn format_speed(bits: u64) -> String {
    if bits < 1000 {
        format!("{}b", bits)
    } else if bits < 1_000_000 {
        format!("{:.1}K", bits as f64 / 1000.0)
    } else {
        format!("{:.1}M", bits as f64 / 1_000_000.0)
    }
}

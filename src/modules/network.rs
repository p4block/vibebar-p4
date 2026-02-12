use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Popover, EventControllerMotion, GestureClick};
use std::time::Duration;
use std::fs;
use std::process::Command;

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

    let label = Label::builder()
        .label("  ...")
        .build();
    label.add_css_class("network-label");
    module_box.append(&label);

    let popover = Popover::builder()
        .position(gtk4::PositionType::Bottom)
        .autohide(false)
        .has_arrow(true)
        .build();
    popover.set_parent(&module_box);
    
    let popover_vbox = Box::new(Orientation::Vertical, 4);
    popover_vbox.set_margin_start(10);
    popover_vbox.set_margin_end(10);
    popover_vbox.set_margin_top(10);
    popover_vbox.set_margin_bottom(10);
    popover.set_child(Some(&popover_vbox));

    let pop_title = Label::builder().halign(gtk4::Align::Start).build();
    let pop_ip = Label::builder().halign(gtk4::Align::Start).build();
    let pop_wifi = Label::builder().halign(gtk4::Align::Start).build();
    let pop_stats = Label::builder().halign(gtk4::Align::Start).build();
    
    pop_title.add_css_class("network-popover-title");
    pop_ip.add_css_class("network-popover-info");
    pop_wifi.add_css_class("network-popover-info");
    pop_stats.add_css_class("network-popover-stats");
    
    popover_vbox.append(&pop_title);
    popover_vbox.append(&pop_ip);
    popover_vbox.append(&pop_wifi);
    popover_vbox.append(&pop_stats);

    // Click gesture for nmtui
    let click_gesture = GestureClick::new();
    click_gesture.set_button(1); // Left click
    click_gesture.connect_pressed(|_, _, _, _| {
        let _ = Command::new("footclient")
            .arg("nmtui")
            .spawn();
    });
    module_box.add_controller(click_gesture);

    // Hover gesture for popover
    let motion_controller = EventControllerMotion::new();
    let p_enter = popover.clone();
    motion_controller.connect_enter(move |_, _, _| {
        p_enter.popup();
    });
    let p_leave = popover.clone();
    motion_controller.connect_leave(move |_| {
        p_leave.popdown();
    });
    module_box.add_controller(motion_controller);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, NetworkInfo)>();
    
    let b = label.clone();
    let p_title = pop_title.clone();
    let p_ip = pop_ip.clone();
    let p_wifi = pop_wifi.clone();
    let p_stats = pop_stats.clone();
    
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some((display_text, info)) = rx.recv().await {
            b.set_label(&display_text);
            
            p_title.set_markup(&format!("<b>{} @ {}</b>", info.ssid.as_deref().unwrap_or("Internet"), info.conn_type));
            p_ip.set_text(&format!("IP: {}", info.ip_cidr));
            
            if let (Some(s), Some(f)) = (info.strength, info.frequency) {
                p_wifi.set_visible(true);
                p_wifi.set_text(&format!("Strength: {}%  Freq: {}MHz", s, f));
            } else {
                p_wifi.set_visible(false);
            }
            
            p_stats.set_text(&format!("Up: {:>7}bps  Down: {:>7}bps", 
                format_speed(info.up_speed), format_speed(info.down_speed)));
        }
    });

    std::thread::spawn(move || {
        let mut last_rx = 0u64;
        let mut last_tx = 0u64;
        let mut last_iface = String::new();

        loop {
            let mut info = NetworkInfo {
                interface: "none".to_string(),
                ssid: None,
                conn_type: "Disconnected".to_string(),
                ip_cidr: "0.0.0.0/0".to_string(),
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
                        let metric = parts[6].parse::<u32>().unwrap_or(u32::MAX);
                        if metric < min_metric {
                            min_metric = metric;
                            best_iface = Some(parts[0].to_string());
                        }
                    }
                }

                if let Some(iface) = best_iface {
                    // Reset bits counters if interface changed
                    if iface != last_iface {
                        last_rx = 0;
                        last_tx = 0;
                        last_iface = iface.clone();
                    }
                    
                    info.interface = iface.clone();
                    let is_wifi = fs::metadata(format!("/sys/class/net/{}/wireless", iface)).is_ok() ||
                                  fs::metadata(format!("/sys/class/net/{}/phy80211", iface)).is_ok();
                    
                    info.conn_type = if is_wifi { "WiFi".to_string() } else { "Ethernet".to_string() };
                    
                    if is_wifi {
                        if let Ok(output) = Command::new("iwgetid").arg("-r").output() {
                            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            if !s.is_empty() { info.ssid = Some(s); }
                        }
                        
                        // Signal Strength & Frequency
                        if let Ok(wireless) = fs::read_to_string("/proc/net/wireless") {
                            for line in wireless.lines().skip(2) {
                                if line.contains(&iface) {
                                    let parts: Vec<&str> = line.split_whitespace().collect();
                                    if let Some(lvl) = parts.get(3) {
                                        let level = lvl.trim_end_matches('.').parse::<f32>().unwrap_or(0.0);
                                        // Simple mapping: -100 to -50 -> 0 to 100
                                        let strength = ((level + 100.0) * 2.0).clamp(0.0, 100.0) as u32;
                                        info.strength = Some(strength);
                                    }
                                }
                            }
                        }
                        
                        // Frequency using iwconfig (fallback if needed)
                        if let Ok(output) = Command::new("iwconfig").arg(&iface).output() {
                            let s = String::from_utf8_lossy(&output.stdout);
                            if let Some(pos) = s.find("Frequency:") {
                                let sub = &s[pos + 10..];
                                let freq_str = sub.split_whitespace().next().unwrap_or("0");
                                info.frequency = Some((freq_str.parse::<f32>().unwrap_or(0.0) * 1000.0) as u32);
                            }
                        }
                    }

                    // IP & CIDR
                    if let Ok(output) = Command::new("ip").arg("-4").arg("addr").arg("show").arg(&iface).output() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        for line in stdout.lines() {
                            if line.contains("inet ") {
                                let parts: Vec<&str> = line.split_whitespace().collect();
                                if let Some(pos) = parts.iter().position(|&r| r == "inet") {
                                    if let Some(addr) = parts.get(pos + 1) {
                                        info.ip_cidr = addr.to_string();
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    // Bandwidth
                    if let Ok(dev) = fs::read_to_string("/proc/net/dev") {
                        for line in dev.lines() {
                            if line.contains(&iface) {
                                let parts: Vec<&str> = line.split_whitespace().collect();
                                // Parse safely to avoid index errors if the line format is weird
                                let current_rx = parts.get(1).and_then(|p| p.parse::<u64>().ok()).unwrap_or(0);
                                let current_tx = parts.get(9).and_then(|p| p.parse::<u64>().ok()).unwrap_or(0);
                                
                                if last_rx > 0 {
                                    info.down_speed = current_rx.saturating_sub(last_rx) * 8; 
                                    info.up_speed = current_tx.saturating_sub(last_tx) * 8;
                                }
                                
                                last_rx = current_rx;
                                last_tx = current_tx;
                                break;
                            }
                        }
                    }
                } else {
                    // No default interface
                    last_rx = 0;
                    last_tx = 0;
                    last_iface.clear();
                }
            }

            let icon = if info.conn_type == "WiFi" { "" } else { "" };
            let display_text = if info.interface == "none" {
                "  Disconnected".to_string()
            } else {
                let ssid_part = info.ssid.as_deref().map(|s| format!("{} ", s)).unwrap_or_default();
                format!("{}  {}{}", icon, ssid_part, info.ip_cidr.split('/').next().unwrap_or("0.0.0.0"))
            };
            
            let _ = tx.send((display_text, info));
            std::thread::sleep(Duration::from_secs(1));
        }
    });
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

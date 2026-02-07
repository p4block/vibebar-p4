use gtk4::prelude::*;
use gtk4::Label;
use std::time::Duration;
use std::fs;

fn format_bandwidth(kb: u64) -> String {
    let mbps = (kb as f64 * 8.0) / 1024.0;
    if mbps >= 1000.0 {
        format!("{:>4.1}G", mbps / 1024.0)
    } else {
        format!("{:>4.1}M", mbps)
    }
}

pub fn init(container: &gtk4::Box, interface: &str) {
    let label = Label::builder()
        .label(" ...")
        .build();
    label.add_css_class("network-module");
    container.append(&label);

    let iface = interface.to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    
    let l = label.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(txt) = rx.recv().await {
            l.set_label(&txt);
        }
    });

    std::thread::spawn(move || {
        let mut prev_rx = 0;
        let mut prev_tx = 0;
        loop {
            if let Ok(content) = fs::read_to_string("/proc/net/dev") {
                for line in content.lines() {
                    if line.contains(&iface) {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() > 9 {
                            let curr_rx = parts[1].parse::<u64>().unwrap_or(0);
                            let curr_tx = parts[9].parse::<u64>().unwrap_or(0);
                            
                            if prev_rx > 0 && prev_tx > 0 {
                                let rx_kb = (curr_rx - prev_rx) / 1024;
                                let tx_kb = (curr_tx - prev_tx) / 1024;
                                let rx_str = format_bandwidth(rx_kb);
                                let tx_str = format_bandwidth(tx_kb);
                                let _ = tx.send(format!(" {} {}", rx_str, tx_str));
                            }
                            
                            prev_rx = curr_rx;
                            prev_tx = curr_tx;
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

use crate::modules::ui;
use gtk4::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use sysinfo::System;

fn find_cpu_hwmon() -> Option<PathBuf> {
    let hwmon_dir = PathBuf::from("/sys/class/hwmon");
    let entries = std::fs::read_dir(hwmon_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = std::fs::read_to_string(path.join("name")).ok()?;
        let cpu_sensors = [
            "k10temp", "coretemp", "zenpower", "k8temp", "fam15h_power",
        ];
        if cpu_sensors.contains(&name.trim()) && path.join("temp1_input").exists() {
            return Some(path);
        }
    }
    None
}

pub fn init(container: &gtk4::Box) {
    let btn = ui::empty_button();
    container.append(&btn);

    btn.connect_clicked(|_| {
        let _ = Command::new("footclient").arg("-e").arg("btop").spawn();
    });

    let mut sys = System::new();
    let mut last_label = String::new();

    let cpu_hwmon = find_cpu_hwmon();

    let mut update = move || {
        sys.refresh_cpu_usage();
        sys.refresh_cpu_specifics(sysinfo::CpuRefreshKind::nothing().with_frequency());

        // CPU Frequency (max)
        let max_freq = sys.cpus().iter().map(|c| c.frequency()).max().unwrap_or(0);
        let ghz = max_freq as f64 / 1000.0;

        // Temperature (Direct sysfs read via auto-detected hwmon)
        let temp = cpu_hwmon
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p.join("temp1_input")).ok())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .map(|t| t / 1000.0)
            .unwrap_or(0.0);

        // Unicode bars for each core
        let bars: String = sys
            .cpus()
            .iter()
            .map(|cpu| {
                let usage = cpu.cpu_usage();
                match usage {
                    u if u < 12.5 => " ",
                    u if u < 25.0 => "▂",
                    u if u < 37.5 => "▃",
                    u if u < 50.0 => "▄",
                    u if u < 62.5 => "▅",
                    u if u < 75.0 => "▆",
                    u if u < 87.5 => "▇",
                    _ => "█",
                }
            })
            .collect();

        ui::set_button_label(
            &btn,
            &mut last_label,
            format!("  {:.1}GHz {:.0}°C {}", ghz, temp, bars),
        );
    };

    update();

    glib::timeout_add_local(Duration::from_secs(2), move || {
        update();
        glib::ControlFlow::Continue
    });
}

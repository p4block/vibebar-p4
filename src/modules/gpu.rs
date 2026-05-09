use crate::modules::ui;
use gtk4::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const DEVICE_BASE: &str = "/sys/class/drm/card1/device";

pub fn init(container: &gtk4::Box) {
    let btn = ui::button("󰢮  ...");
    container.append(&btn);

    btn.connect_clicked(|_| {
        let _ = Command::new("lact").spawn();
    });

    let mut last_label = String::new();
    let mut update = move || {
        let gpu_usage = std::fs::read_to_string(format!("{}/gpu_busy_percent", DEVICE_BASE))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);

        let vram_usage = read_vram_usage_percent(DEVICE_BASE).unwrap_or(0);

        let mut freq = 0.0;
        let mut power_watts = 0.0;

        if let Some(hwmon_path) = find_hwmon_path(DEVICE_BASE) {
            if let Ok(s) = std::fs::read_to_string(hwmon_path.join("freq1_input")) {
                if let Ok(f) = s.trim().parse::<u32>() {
                    freq = f as f64 / 1_000_000_000.0;
                }
            }

            if let Ok(s) = std::fs::read_to_string(hwmon_path.join("power1_average")) {
                if let Ok(p) = s.trim().parse::<u32>() {
                    power_watts = p as f64 / 1_000_000.0;
                }
            }
        }

        ui::set_button_label(
            &btn,
            &mut last_label,
            format!(
                "󰢮  {}% {:.1}GHz {:.1}W {}%",
                gpu_usage, freq, power_watts, vram_usage
            ),
        );
    };

    update();

    glib::timeout_add_local(Duration::from_secs(2), move || {
        update();
        glib::ControlFlow::Continue
    });
}

fn find_hwmon_path(device_base: &str) -> Option<PathBuf> {
    let hwmon_dir = format!("{}/hwmon", device_base);
    let entries = std::fs::read_dir(hwmon_dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("power1_average").exists() {
            return Some(path);
        }
    }
    None
}

fn read_vram_usage_percent(device_base: &str) -> Option<u32> {
    let used = read_sysfs_u64(&format!("{}/mem_info_vram_used", device_base))?;
    let total = read_sysfs_u64(&format!("{}/mem_info_vram_total", device_base))?;
    if total == 0 {
        return None;
    }

    Some(((used as f64 / total as f64) * 100.0).round() as u32)
}

fn read_sysfs_u64(path: &str) -> Option<u64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
}

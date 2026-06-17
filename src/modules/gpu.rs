use crate::modules::ui;
use gtk4::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

fn find_gpu_device_base() -> Option<PathBuf> {
    let drm_dir = PathBuf::from("/sys/class/drm");
    let entries = std::fs::read_dir(drm_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name()?.to_str()?;
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let device = path.join("device");
        if device.exists() && device.join("gpu_busy_percent").exists() {
            return Some(device);
        }
    }
    None
}

pub fn init(container: &gtk4::Box) {
    let btn = ui::button("󰢮  ...");
    container.append(&btn);

    btn.connect_clicked(|_| {
        let _ = Command::new("lact").spawn();
    });

    let device_base = find_gpu_device_base();

    let mut last_label = String::new();
    let mut update = move || {
        let Some(ref base) = device_base else {
            return;
        };

        let gpu_usage = std::fs::read_to_string(base.join("gpu_busy_percent"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);

        let vram_usage = read_vram_usage_percent(base).unwrap_or(0);

        let mut freq = 0.0;
        let mut power_watts = 0.0;

        if let Some(hwmon_path) = find_hwmon_path(base) {
            if let Ok(s) = std::fs::read_to_string(hwmon_path.join("freq1_input"))
                && let Ok(f) = s.trim().parse::<u32>()
            {
                freq = f as f64 / 1_000_000_000.0;
            }

            if let Ok(s) = std::fs::read_to_string(hwmon_path.join("power1_average"))
                && let Ok(p) = s.trim().parse::<u32>()
            {
                power_watts = p as f64 / 1_000_000.0;
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

fn find_hwmon_path(device_base: &PathBuf) -> Option<PathBuf> {
    let hwmon_dir = device_base.join("hwmon");
    let entries = std::fs::read_dir(hwmon_dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("power1_average").exists() {
            return Some(path);
        }
    }
    None
}

fn read_vram_usage_percent(device_base: &PathBuf) -> Option<u32> {
    let used = read_sysfs_u64(&device_base.join("mem_info_vram_used"))?;
    let total = read_sysfs_u64(&device_base.join("mem_info_vram_total"))?;
    if total == 0 {
        return None;
    }

    Some(((used as f64 / total as f64) * 100.0).round() as u32)
}

fn read_sysfs_u64(path: &PathBuf) -> Option<u64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
}

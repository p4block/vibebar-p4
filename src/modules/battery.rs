use crate::modules::ui;
use gtk4::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
struct BatteryInfo {
    capacity: u32,
    status: String,
}

pub fn init(container: &gtk4::Box) {
    let (btn, label) = ui::label_button("󰁹 ...%");
    btn.set_visible(false);
    container.append(&btn);

    let batteries = find_batteries();
    if batteries.is_empty() {
        return;
    }

    let mut last_label = String::new();
    let mut update = move || {
        let Some(info) = read_battery_info(&batteries) else {
            btn.set_visible(false);
            return;
        };

        btn.set_visible(true);
        ui::set_label(&label, &mut last_label, format_battery(&info));
    };

    update();

    glib::timeout_add_local(Duration::from_secs(30), move || {
        update();
        glib::ControlFlow::Continue
    });
}

fn find_batteries() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| matches!(name, "BAT0" | "BAT1"))
                .unwrap_or(false)
        })
        .filter(|path| {
            std::fs::read_to_string(path.join("type"))
                .map(|s| s.trim() == "Battery")
                .unwrap_or(false)
        })
        .collect()
}

fn read_battery_info(paths: &[PathBuf]) -> Option<BatteryInfo> {
    let mut total_capacity = 0;
    let mut count = 0;
    let mut statuses = Vec::new();

    for path in paths {
        let capacity = std::fs::read_to_string(path.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())?;
        let status = std::fs::read_to_string(path.join("status"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        total_capacity += capacity.min(100);
        count += 1;
        statuses.push(status);
    }

    if count == 0 {
        return None;
    }

    let status = if statuses.iter().any(|s| s == "Charging") {
        "Charging"
    } else if statuses.iter().any(|s| s == "Discharging") {
        "Discharging"
    } else if statuses.iter().any(|s| s == "Full") {
        "Full"
    } else {
        "Unknown"
    };

    Some(BatteryInfo {
        capacity: total_capacity / count,
        status: status.to_string(),
    })
}

fn format_battery(info: &BatteryInfo) -> String {
    let icon = if info.status == "Charging" {
        "󰂄"
    } else {
        match info.capacity {
            0..=10 => "󰂎",
            11..=25 => "󰁻",
            26..=50 => "󰁾",
            51..=75 => "󰂁",
            _ => "󰁹",
        }
    };

    format!("{}  {}%", icon, info.capacity)
}

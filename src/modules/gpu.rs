use crate::modules::ui;
use gtk4::prelude::*;
use std::process::Command;
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    let btn = ui::button("󰢮  ...");
    container.append(&btn);

    btn.connect_clicked(|_| {
        let _ = Command::new("lact").spawn();
    });

    let mut last_label = String::new();
    let mut update = move || {
        let gpu_usage = std::fs::read_to_string("/sys/class/drm/card1/device/gpu_busy_percent")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);

        let freq = std::fs::read_to_string("/sys/class/drm/card1/device/hwmon/hwmon7/freq1_input")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .map(|f| f as f64 / 1000.0 / 1000.0 / 1000.0)
            .unwrap_or(0.0);

        let power_raw =
            std::fs::read_to_string("/sys/class/drm/card1/device/hwmon/hwmon7/power1_average")
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(0);
        let power_watts = power_raw as f64 / 1_000_000.0;

        ui::set_button_label(
            &btn,
            &mut last_label,
            format!("󰢮  {}% {:.1}GHz {:.1}W", gpu_usage, freq, power_watts),
        );
    };

    update();

    glib::timeout_add_local(Duration::from_secs(2), move || {
        update();
        glib::ControlFlow::Continue
    });
}

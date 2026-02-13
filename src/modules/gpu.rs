use gtk4::prelude::*;
use gtk4::{Button, GestureClick};
use std::process::Command;
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    let btn = Button::builder().label("󰢮  ...").build();
    btn.add_css_class("btn");
    container.append(&btn);

    let gesture = GestureClick::new();
    gesture.set_button(1);
    gesture.connect_pressed(|_, _, _, _| {
        let _ = Command::new("footclient").arg("-e").arg("btop").spawn();
    });
    btn.add_controller(gesture);

    glib::timeout_add_local(Duration::from_secs(2), move || {
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

        btn.set_label(&format!(
            "󰢮  {}% {:.1}GHz {:.1}W",
            gpu_usage, freq, power_watts
        ));

        glib::ControlFlow::Continue
    });
}

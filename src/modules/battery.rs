use gtk4::prelude::*;
use gtk4::Label;
use std::time::Duration;
use std::fs;

pub fn init(container: &gtk4::Box) {
    let label = Label::builder()
        .label(" ...")
        .build();
    label.set_widget_name("battery-module");
    container.append(&label);

    glib::timeout_add_local(Duration::from_secs(5), move || {
        let capacity = fs::read_to_string("/sys/class/power_supply/BAT1/capacity")
            .unwrap_or_else(|_| "0".to_string())
            .trim()
            .parse::<i32>()
            .unwrap_or(0);

        let status = fs::read_to_string("/sys/class/power_supply/BAT1/status")
            .unwrap_or_else(|_| "Unknown".to_string())
            .trim()
            .to_string();

        let icon = match capacity {
            c if c > 90 => "",
            c if c > 60 => "",
            c if c > 40 => "",
            c if c > 20 => "",
            _ => "",
        };

        label.set_label(&format!("{}  {}%", icon, capacity));

        if status == "Charging" {
            label.add_css_class("charging");
        } else {
            label.remove_css_class("charging");
        }

        if capacity <= 5 && status != "Charging" {
            label.add_css_class("critical");
        } else {
            label.remove_css_class("critical");
        }

        glib::ControlFlow::Continue
    });
}

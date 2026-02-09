use gtk4::prelude::*;
use gtk4::{Label, EventControllerScroll, EventControllerScrollFlags};
use std::time::Duration;
use std::fs;
use std::process::Command;

pub fn init(container: &gtk4::Box) {
    let label = Label::builder()
        .label("  ...%")
        .build();
    label.set_widget_name("brightness-module");
    container.append(&label);

    let scroll = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    label.add_controller(scroll.clone());

    scroll.connect_scroll(move |_, _, dy| {
        if dy < 0.0 {
            let _ = Command::new("brightnessctl")
                .arg("set")
                .arg("5%+")
                .spawn();
        } else if dy > 0.0 {
            let _ = Command::new("brightnessctl")
                .arg("set")
                .arg("5%-")
                .spawn();
        }
        glib::Propagation::Stop
    });

    glib::timeout_add_local(Duration::from_millis(500), move || {
        let brightness = fs::read_to_string("/sys/class/backlight/amdgpu_bl1/brightness")
            .unwrap_or_else(|_| "0".to_string())
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0);
        
        let max_brightness = fs::read_to_string("/sys/class/backlight/amdgpu_bl1/max_brightness")
            .unwrap_or_else(|_| "1".to_string())
            .trim()
            .parse::<f64>()
            .unwrap_or(1.0);

        let perc = (brightness / max_brightness * 100.0).round() as i32;
        label.set_label(&format!("  {}%", perc));
        
        glib::ControlFlow::Continue
    });
}

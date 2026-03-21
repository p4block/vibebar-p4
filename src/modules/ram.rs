use gtk4::Button;
use gtk4::prelude::*;
use std::time::Duration;
use sysinfo::System;

pub fn init(container: &gtk4::Box) {
    let _label = Button::builder().label("  ...%").build();
    let btn = Button::new();
    btn.add_css_class("btn");
    container.append(&btn);

    let mut sys = System::new();
    let mut last_label = String::new();

    glib::timeout_add_local(Duration::from_secs(30), move || {
        sys.refresh_memory();
        let used = sys.used_memory();
        let total = sys.total_memory();
        let perc = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let new_label = format!("  {:.0}%", perc);
        if new_label != last_label {
            btn.set_label(&new_label);
            last_label = new_label;
        }
        glib::ControlFlow::Continue
    });
}

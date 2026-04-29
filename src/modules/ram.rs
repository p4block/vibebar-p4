use crate::modules::ui;
use gtk4::prelude::*;
use std::time::Duration;
use sysinfo::System;

pub fn init(container: &gtk4::Box) {
    let btn = ui::empty_button();
    container.append(&btn);

    let mut sys = System::new();
    let mut last_label = String::new();

    let mut update = move || {
        sys.refresh_memory();
        let used = sys.used_memory();
        let total = sys.total_memory();
        let perc = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        ui::set_button_label(&btn, &mut last_label, format!("  {:.0}%", perc));
    };

    update();

    glib::timeout_add_local(Duration::from_secs(30), move || {
        update();
        glib::ControlFlow::Continue
    });
}

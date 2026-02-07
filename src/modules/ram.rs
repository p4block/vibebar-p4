use gtk4::prelude::*;
use gtk4::Label;
use std::time::Duration;
use sysinfo::System;

pub fn init(container: &gtk4::Box) {
    let label = Label::builder()
        .label(" ...%")
        .build();
    container.append(&label);

    let mut sys = System::new_all();

    glib::timeout_add_local(Duration::from_secs(2), move || {
        sys.refresh_memory();
        let used = sys.used_memory();
        let total = sys.total_memory();
        let perc = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };
        
        label.set_label(&format!(" {:.0}%", perc));
        glib::ControlFlow::Continue
    });
}

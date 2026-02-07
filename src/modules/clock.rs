use gtk4::prelude::*;
use gtk4::Label;
use chrono::Local;
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    let label = Label::builder()
        .label(&Local::now().format("%a %d %b %H:%M").to_string())
        .build();

    container.append(&label);

    // Update every minute (on the minute would be better, but 1s interval to check is fine/cheap)
    glib::timeout_add_local(Duration::from_secs(1), move || {
        label.set_label(&Local::now().format("%a %d %b %H:%M").to_string());
        glib::ControlFlow::Continue
    });
}

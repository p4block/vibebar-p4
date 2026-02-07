use gtk4::prelude::*;
use gtk4::Label;
use std::time::Duration;
use glib;

pub fn init(container: &gtk4::Box, path: &str, label_prefix: &str) {
    let label = Label::builder()
        .label(&format!("{} ...", label_prefix))
        .build();
    container.append(&label);

    let path_clone = path.to_string();
    let prefix_clone = label_prefix.to_string();
    let label_clone = label.clone();
    let update = move || {
        if let Ok(stat) = nix::sys::statvfs::statvfs(path_clone.as_str()) {
            let free_bytes = stat.blocks_available() * stat.fragment_size();
            let free_gb = free_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            label_clone.set_label(&format!("{} {:.1}GB", prefix_clone, free_gb));
        }
    };

    update();

    glib::timeout_add_local(Duration::from_secs(60), move || {
        update();
        glib::ControlFlow::Continue
    });
}

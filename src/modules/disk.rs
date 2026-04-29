use glib;
use gtk4::Button;
use gtk4::prelude::*;
use std::time::Duration;

pub fn init(container: &gtk4::Box, path: &str, label_prefix: &str) {
    let btn = Button::builder()
        .label(format!("{} ...", label_prefix))
        .build();
    btn.add_css_class("btn");
    container.append(&btn);

    let path_clone = path.to_string();
    let prefix_clone = label_prefix.to_string();
    let btn_clone = btn.clone();

    let last_label = std::rc::Rc::new(std::cell::RefCell::new(String::new()));

    let update = {
        let last_label = last_label.clone();
        let path_clone = path_clone.clone();
        let prefix_clone = prefix_clone.clone();
        let btn_clone = btn_clone.clone();
        move || {
            if let Ok(stat) = nix::sys::statvfs::statvfs(path_clone.as_str()) {
                let free_bytes = stat.blocks_available() * stat.fragment_size();
                let free_gb = free_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                let new_label = format!("{} {:.1}GB", prefix_clone, free_gb);
                let mut cache = last_label.borrow_mut();
                if new_label != *cache {
                    btn_clone.set_label(&new_label);
                    *cache = new_label;
                }
            }
        }
    };

    update();

    glib::timeout_add_local(Duration::from_secs(300), move || {
        update();
        glib::ControlFlow::Continue
    });
}

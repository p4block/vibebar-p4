use crate::modules::ui;
use gtk4::prelude::*;
use gtk4::{EventControllerScroll, EventControllerScrollFlags};
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    let (btn, label) = ui::label_button("󰃠 ...%");
    btn.set_visible(false);
    container.append(&btn);

    let Some(device) = find_backlight_device() else {
        return;
    };

    let scroll = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    let scroll_accum = Rc::new(RefCell::new(0.0));
    let scroll_accum_clone = scroll_accum.clone();

    scroll.connect_scroll(move |_, _, dy| {
        let mut acc = scroll_accum_clone.borrow_mut();
        *acc += dy;

        while *acc <= -1.0 {
            adjust_brightness("5%+");
            *acc += 1.0;
        }

        while *acc >= 1.0 {
            adjust_brightness("5%-");
            *acc -= 1.0;
        }

        glib::Propagation::Stop
    });
    btn.add_controller(scroll);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u32>();

    let mut last_label = String::new();
    let b = btn.clone();
    let label_clone = label.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(percent) = rx.recv().await {
            b.set_visible(true);
            ui::set_label(&label_clone, &mut last_label, format!("󰃠  {}%", percent));
        }
    });

    // Initial read so the bar shows a value before the first inotify event.
    if let Some(percent) = read_brightness_percent(&device) {
        btn.set_visible(true);
        let _ = tx.send(percent);
    }

    // Reactive updates: watch the sysfs brightness file with inotify and
    // refresh the label only when it changes. Falls back to polling if the
    // kernel/driver does not support inotify on this attribute.
    let device_watcher = device.clone();
    let tx_watcher = tx.clone();
    std::thread::spawn(move || watch_brightness(&device_watcher, tx_watcher));
}

fn watch_brightness(device: &Path, tx: tokio::sync::mpsc::UnboundedSender<u32>) {
    let brightness_path = device.join("brightness");

    // Reactive path: watch the sysfs brightness attribute for changes.
    // Drivers call sysfs_notify() on writes, so this fires for hardware-key,
    // brightnessctl, and direct-sysfs changes alike.
    let inotify = Inotify::init(InitFlags::empty()).and_then(|inotify| {
        inotify
            .add_watch(&brightness_path, AddWatchFlags::IN_MODIFY)
            .map(|_| inotify)
    });

    match inotify {
        Ok(inotify) => loop {
            // Blocks until the brightness attribute is modified.
            if inotify.read_events().is_err() {
                break;
            }
            if let Some(percent) = read_brightness_percent(device) {
                let _ = tx.send(percent);
            }
        },
        Err(_) => loop {
            std::thread::sleep(Duration::from_secs(5));
            if let Some(percent) = read_brightness_percent(device) {
                let _ = tx.send(percent);
            }
        },
    }
}

fn adjust_brightness(value: &str) {
    let _ = std::process::Command::new("brightnessctl")
        .arg("set")
        .arg(value)
        .spawn();
}

fn read_brightness_percent(device: &Path) -> Option<u32> {
    let current = read_u64(device.join("brightness"))?;
    let max = read_u64(device.join("max_brightness"))?;
    if max == 0 {
        return None;
    }

    Some(((current as f64 / max as f64) * 100.0).round() as u32)
}

fn find_backlight_device() -> Option<PathBuf> {
    let entries = std::fs::read_dir("/sys/class/backlight").ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .next()
}

fn read_u64(path: PathBuf) -> Option<u64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
}

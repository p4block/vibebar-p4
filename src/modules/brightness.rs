use crate::modules::ui;
use gtk4::prelude::*;
use gtk4::{EventControllerScroll, EventControllerScrollFlags};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    let (btn, label) = ui::label_button("󰃠 ...%");
    btn.set_visible(false);
    container.append(&btn);

    if !brightness_available() {
        return;
    }

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

    let mut last_label = String::new();
    let mut update = move || {
        let Some(percent) = read_brightness_percent() else {
            btn.set_visible(false);
            return;
        };

        btn.set_visible(true);
        ui::set_label(&label, &mut last_label, format!("󰃠  {}%", percent));
    };

    update();

    glib::timeout_add_local(Duration::from_secs(2), move || {
        update();
        glib::ControlFlow::Continue
    });
}

fn brightness_available() -> bool {
    command_exists("brightnessctl") || find_backlight_device().is_some()
}

fn adjust_brightness(value: &str) {
    let _ = std::process::Command::new("brightnessctl")
        .arg("set")
        .arg(value)
        .spawn();
}

fn read_brightness_percent() -> Option<u32> {
    if let Some(percent) = read_brightnessctl_percent() {
        return Some(percent);
    }

    let device = find_backlight_device()?;
    let current = read_u64(device.join("brightness"))?;
    let max = read_u64(device.join("max_brightness"))?;
    if max == 0 {
        return None;
    }

    Some(((current as f64 / max as f64) * 100.0).round() as u32)
}

fn read_brightnessctl_percent() -> Option<u32> {
    let output = std::process::Command::new("brightnessctl")
        .arg("-m")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .split(',')
        .find_map(|part| part.strip_suffix('%'))
        .and_then(|part| part.parse::<u32>().ok())
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

fn command_exists(command: &str) -> bool {
    std::process::Command::new(command)
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

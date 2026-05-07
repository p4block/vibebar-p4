use crate::modules::ui;
use gtk4::prelude::*;
use std::process::Command;
use std::time::Duration;

const FALLBACK_PROFILES: [&str; 3] = ["power-saver", "balanced", "performance"];

pub fn init(container: &gtk4::Box) {
    let btn = ui::button("󰓅 ...");
    btn.set_visible(false);
    container.append(&btn);

    if read_profile().is_none() {
        return;
    }

    btn.connect_clicked(|_| {
        if let Some(current) = read_profile() {
            let profiles = read_profiles();
            let next = next_profile(&current, &profiles);
            let _ = Command::new("powerprofilesctl")
                .arg("set")
                .arg(next)
                .spawn();
        }
    });

    let mut last_label = String::new();
    let mut update = move || {
        let Some(profile) = read_profile() else {
            btn.set_visible(false);
            return;
        };

        btn.set_visible(true);
        ui::set_button_label(&btn, &mut last_label, format_profile(&profile));
    };

    update();

    glib::timeout_add_local(Duration::from_secs(10), move || {
        update();
        glib::ControlFlow::Continue
    });
}

fn read_profile() -> Option<String> {
    let output = Command::new("powerprofilesctl").arg("get").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let profile = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if profile.is_empty() {
        None
    } else {
        Some(profile)
    }
}

fn read_profiles() -> Vec<String> {
    let output = Command::new("powerprofilesctl").arg("list").output().ok();
    let Some(output) = output.filter(|output| output.status.success()) else {
        return FALLBACK_PROFILES
            .iter()
            .map(|profile| profile.to_string())
            .collect();
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let profiles: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_start_matches('*').trim();
            line.strip_suffix(':').map(str::to_string)
        })
        .collect();

    if profiles.is_empty() {
        FALLBACK_PROFILES
            .iter()
            .map(|profile| profile.to_string())
            .collect()
    } else {
        profiles
    }
}

fn next_profile<'a>(current: &str, profiles: &'a [String]) -> &'a str {
    match profiles.iter().position(|profile| profile == current) {
        Some(index) => &profiles[(index + 1) % profiles.len()],
        None => profiles
            .iter()
            .find(|profile| profile.as_str() == "balanced")
            .map(String::as_str)
            .unwrap_or("balanced"),
    }
}

fn format_profile(profile: &str) -> String {
    match profile {
        "power-saver" => "󰌪  save".to_string(),
        "performance" => "󰓅  perf".to_string(),
        _ => "󰾅  bal".to_string(),
    }
}

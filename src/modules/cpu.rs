use gtk4::prelude::*;
use gtk4::Label;
use std::time::Duration;
use sysinfo::{System, Components};

pub fn init(container: &gtk4::Box) {
    let label = Label::builder()
        .label(" ...")
        .build();
    container.append(&label);

    let mut sys = System::new();
    let mut components = Components::new();

    glib::timeout_add_local(Duration::from_secs(2), move || {
        sys.refresh_cpu_usage();
        sys.refresh_cpu_specifics(sysinfo::CpuRefreshKind::nothing().with_frequency());
        components.refresh(false);

        // CPU Frequency (max)
        let max_freq = sys.cpus().iter()
            .map(|c| c.frequency())
            .max()
            .unwrap_or(0);
        let ghz = max_freq as f64 / 1000.0;

        // Temperature (from first component for now)
        let mut temp = 0.0;
        for c in components.iter() {
            if c.label().to_lowercase().contains("cpu") || c.label().to_lowercase().contains("core") {
                temp = c.temperature().unwrap_or(0.0);
                break;
            }
        }

        // Unicode bars for each core
        let bars: String = sys.cpus().iter().map(|cpu| {
            let usage = cpu.cpu_usage();
            match usage {
                u if u < 12.5 => " ",
                u if u < 25.0 => "▂",
                u if u < 37.5 => "▃",
                u if u < 50.0 => "▄",
                u if u < 62.5 => "▅",
                u if u < 75.0 => "▆",
                u if u < 87.5 => "▇",
                _ => "█",
            }
        }).collect();

        label.set_label(&format!(" {} {:.1}GHz {:.0}°C", bars, ghz, temp));
        glib::ControlFlow::Continue
    });
}

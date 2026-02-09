use gtk4::prelude::*;
use gtk4::Button;
use pulsectl::controllers::SinkController;
use pulsectl::controllers::DeviceControl;
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    let btn = Button::builder()
        .label(" ...%")
        .build();
    btn.set_widget_name("volume-btn");
    container.append(&btn);

    btn.connect_clicked(|_| {
        let _ = std::process::Command::new("pactl")
            .arg("set-sink-mute")
            .arg("@DEFAULT_SINK@")
            .arg("toggle")
            .spawn();
    });

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let b = btn.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(vol) = rx.recv().await {
            b.set_label(&vol);
        }
    });

    std::thread::spawn(move || {
        let mut controller = SinkController::create().unwrap();
        loop {
            if let Ok(default_sink) = controller.get_default_device() {
                let vol_val = default_sink.volume.avg().0;
                let perc = (vol_val as f64 / 65536.0 * 100.0) as i32;
                let muted = default_sink.mute;
                let icon = if muted { "" } else { "" };
                let _ = tx.send(format!("{}  {}%", icon, perc));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

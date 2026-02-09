use gtk4::prelude::*;
use gtk4::Button;
use mpris::PlayerFinder;
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    let btn = Button::builder()
        .label("")
        .build();
    btn.set_widget_name("mpris-btn");
    container.append(&btn);

    btn.connect_clicked(|_| {
        let _ = std::process::Command::new("playerctl")
            .arg("play-pause")
            .spawn();
    });

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let b = btn.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(txt) = rx.recv().await {
            b.set_label(&txt);
        }
    });

    std::thread::spawn(move || {
        let finder = PlayerFinder::new().unwrap();
        loop {
            if let Ok(player) = finder.find_active() {
                if let Ok(metadata) = player.get_metadata() {
                    let artist = metadata.artists().map(|a| a.join(", ")).unwrap_or_default();
                    let title = metadata.title().unwrap_or_default();
                    let status = player.get_playback_status().unwrap();
                    let icon = match status {
                        mpris::PlaybackStatus::Playing => "",
                        mpris::PlaybackStatus::Paused => "",
                        _ => "⏹",
                    };
                    let _ = tx.send(format!("{}  {} - {}", icon, artist, title));
                } else {
                    let _ = tx.send("".to_string());
                }
            } else {
                let _ = tx.send("".to_string());
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

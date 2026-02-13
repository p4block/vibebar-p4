use gtk4::Button;
use gtk4::prelude::*;
use mpris::PlayerFinder;
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    let btn = Button::builder().label("").build();
    btn.add_css_class("btn");
    container.append(&btn);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let btn_clone = btn.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(txt) = rx.recv().await {
            btn_clone.set_label(&txt);
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
                    let _ = tx.send(format!("{} {} - {}", icon, artist, title));
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

use gtk4::prelude::*;
use gtk4::Label;
use mpris::PlayerFinder;
use std::time::Duration;

pub fn init(container: &gtk4::Box) {
    let label = Label::builder()
        .label("")
        .build();
    container.append(&label);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let l = label.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(txt) = rx.recv().await {
            l.set_label(&txt);
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

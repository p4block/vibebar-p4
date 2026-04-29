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
        let Ok(finder) = PlayerFinder::new() else {
            return;
        };
        let mut last_text = String::new();
        loop {
            let text = if let Ok(player) = finder.find_active() {
                if let Ok(metadata) = player.get_metadata() {
                    let artist = metadata.artists().map(|a| a.join(", ")).unwrap_or_default();
                    let title = metadata.title().unwrap_or_default();
                    let status = player
                        .get_playback_status()
                        .unwrap_or(mpris::PlaybackStatus::Stopped);
                    let icon = match status {
                        mpris::PlaybackStatus::Playing => "",
                        mpris::PlaybackStatus::Paused => "",
                        _ => "⏹",
                    };
                    truncate_text(format!("{} {} - {}", icon, artist, title), 60)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            if text != last_text {
                let _ = tx.send(text.clone());
                last_text = text;
            }

            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

fn truncate_text(text: String, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text;
    }

    let mut truncated: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    truncated.push_str("...");
    truncated
}

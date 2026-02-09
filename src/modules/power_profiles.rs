use gtk4::prelude::*;
use gtk4::Button;
use zbus::{proxy, Connection};
use futures_util::StreamExt;

#[proxy(
    interface = "net.hadess.PowerProfiles",
    default_service = "net.hadess.PowerProfiles",
    default_path = "/net/hadess/PowerProfiles"
)]
trait PowerProfiles {
    #[zbus(property)]
    fn active_profile(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn set_active_profile(&self, profile: &str) -> zbus::Result<()>;
}

pub fn init(container: &gtk4::Box) {
    let btn = Button::builder()
        .label("")
        .build();
    btn.set_widget_name("power-profile-btn");
    container.append(&btn);

    let (update_tx, mut update_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let b = btn.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(label) = update_rx.recv().await {
            b.set_label(&label);
        }
    });

    let (click_tx, mut click_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    btn.connect_clicked(move |_| {
        let _ = click_tx.send(());
    });

    gtk4::glib::MainContext::default().spawn_local(async move {
        let conn = match Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                let _ = update_tx.send(format!("  Err: {}", e));
                return;
            }
        };
        let proxy = match PowerProfilesProxy::new(&conn).await {
            Ok(p) => p,
            Err(e) => {
                let _ = update_tx.send(format!("  Err: {}", e));
                return;
            }
        };

        let update = |p: String| {
            let label = match p.as_str() {
                "performance" => "",
                "balanced" => "",
                "power-saver" => "",
                _ => "",
            };
            let _ = update_tx.send(label.to_string());
        };

        // Initial update
        if let Ok(p) = proxy.active_profile().await {
            update(p);
        }

        let mut stream = proxy.receive_active_profile_changed().await;
        
        loop {
            tokio::select! {
                Some(_) = stream.next() => {
                    if let Ok(p) = proxy.active_profile().await {
                        update(p);
                    }
                }
                Some(_) = click_rx.recv() => {
                    if let Ok(current) = proxy.active_profile().await {
                        let next = match current.as_str() {
                            "balanced" => "performance",
                            "performance" => "power-saver",
                            _ => "balanced",
                        };
                        let _ = proxy.set_active_profile(next).await;
                    }
                }
                else => break,
            }
        }
    });
}

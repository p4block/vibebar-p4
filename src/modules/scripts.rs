use gtk4::prelude::*;
use gtk4::Label;
use std::time::Duration;
use tokio::process::Command;

pub fn init(container: &gtk4::Box, command: &str, interval_secs: u64, prefix: &str) {
    let label = Label::builder()
        .label(&format!("{} ...", prefix))
        .build();
    container.append(&label);

    let cmd_own = command.to_string();
    let prefix_own = prefix.to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let l = label.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(out) = rx.recv().await {
            let text = if prefix_own.is_empty() {
                out.trim().to_string()
            } else {
                format!("{} {}", prefix_own, out.trim())
            };
            l.set_label(&text);
        }
    });

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            loop {
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(&cmd_own)
                    .output()
                    .await;

                if let Ok(out) = output {
                    let s = String::from_utf8_lossy(&out.stdout).to_string();
                    let display_text = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&s) {
                        json["text"].as_str().unwrap_or(&s).to_string()
                    } else {
                        s.trim().to_string()
                    };
                    let _ = tx.send(display_text);
                }

                tokio::time::sleep(Duration::from_secs(interval_secs)).await;
            }
        });
    });
}

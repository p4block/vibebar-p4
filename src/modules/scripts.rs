use gtk4::prelude::*;
use gtk4::{Button, GestureClick};
use std::process::Command as StdCommand;
use std::time::Duration;
use tokio::process::Command;

pub fn init(
    container: &gtk4::Box,
    command: &str,
    interval_secs: u64,
    prefix: &str,
    click_command: Option<&str>,
) {
    let button = Button::builder().label(format!("{} ...", prefix)).build();

    button.add_css_class("btn");

    container.append(&button);

    if let Some(click_command) = click_command {
        let click_command = click_command.to_string();
        let gesture = GestureClick::new();
        gesture.set_button(1);
        gesture.connect_pressed(move |_, _, _, _| {
            let _ = StdCommand::new("sh").arg("-c").arg(&click_command).spawn();
        });
        button.add_controller(gesture);
    }

    let cmd_own = command.to_string();
    let prefix_own = prefix.to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let b = button.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        while let Some(out) = rx.recv().await {
            let text = if prefix_own.is_empty() {
                out.trim().to_string()
            } else {
                format!("{} {}", prefix_own, out.trim())
            };
            b.set_label(&text);
        }
    });

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let mut last_label = String::new();
            loop {
                let output = Command::new("sh").arg("-c").arg(&cmd_own).output().await;

                if let Ok(out) = output {
                    let s = String::from_utf8_lossy(&out.stdout).to_string();
                    let display_text =
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&s) {
                            json["text"].as_str().unwrap_or(&s).to_string()
                        } else {
                            s.trim().to_string()
                        };

                    if display_text != last_label {
                        let _ = tx.send(display_text.clone());
                        last_label = display_text;
                    }
                }

                tokio::time::sleep(Duration::from_secs(interval_secs)).await;
            }
        });
    });
}

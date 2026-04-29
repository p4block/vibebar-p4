use crate::modules::ui;
use gtk4::GestureClick;
use gtk4::prelude::*;
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
    let button = ui::button(&format!("{} ...", prefix));
    container.append(&button);

    if let Some(click_command) = click_command {
        let click_command = click_command.to_string();
        let gesture = GestureClick::new();
        gesture.set_button(1);
        gesture.connect_pressed(move |_, _, _, _| {
            let _ = spawn_command(&click_command);
        });
        button.add_controller(gesture);
    }

    let cmd_own = command.to_string();
    let prefix_own = prefix.to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let b = button.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        let mut last_label = String::new();
        while let Some(out) = rx.recv().await {
            let text = if prefix_own.is_empty() {
                out.trim().to_string()
            } else {
                format!("{} {}", prefix_own, out.trim())
            };
            ui::set_button_label(&b, &mut last_label, text);
        }
    });

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut last_label = String::new();
            loop {
                if let Some(display_text) = run_command(&cmd_own).await
                    && display_text != last_label
                {
                    let _ = tx.send(display_text.clone());
                    last_label = display_text;
                }

                tokio::time::sleep(Duration::from_secs(interval_secs)).await;
            }
        });
    });
}

async fn run_command(command: &str) -> Option<String> {
    if command.trim() == "checkupdates | wc -l" {
        let output = Command::new("checkupdates").output().await.ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Some(stdout.lines().count().to_string());
    }

    let output = if shell_required(command) {
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await
            .ok()?
    } else {
        let mut parts = command.split_whitespace();
        let program = parts.next()?;
        Command::new(program).args(parts).output().await.ok()?
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Some(parse_display_text(&stdout))
}

fn spawn_command(command: &str) -> std::io::Result<std::process::Child> {
    if shell_required(command) {
        StdCommand::new("sh").arg("-c").arg(command).spawn()
    } else {
        let mut parts = command.split_whitespace();
        let Some(program) = parts.next() else {
            return StdCommand::new("true").spawn();
        };
        StdCommand::new(program).args(parts).spawn()
    }
}

fn parse_display_text(stdout: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout) {
        json["text"].as_str().unwrap_or(stdout).trim().to_string()
    } else {
        stdout.trim().to_string()
    }
}

fn shell_required(command: &str) -> bool {
    command.chars().any(|c| {
        matches!(
            c,
            '|' | '&' | ';' | '<' | '>' | '$' | '`' | '"' | '\'' | '(' | ')'
        )
    })
}

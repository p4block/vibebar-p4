use gtk4::prelude::*;
use gtk4::{Label, Button};
use std::time::Duration;
use tokio::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use tokio::sync::broadcast;

struct ScriptWorker {
    tx: broadcast::Sender<String>,
}

static SCRIPT_MANAGER: OnceLock<Arc<Mutex<HashMap<String, ScriptWorker>>>> = OnceLock::new();

fn get_manager() -> Arc<Mutex<HashMap<String, ScriptWorker>>> {
    SCRIPT_MANAGER.get_or_init(|| Arc::new(Mutex::new(HashMap::new()))).clone()
}

pub fn init(container: &gtk4::Box, command: &str, interval_secs: u64, prefix: &str, click_command: Option<&str>) {
    let label = Label::builder()
        .label(&format!("{} ...", prefix))
        .build();
    
    let click_cmd_own = click_command.map(|s| s.to_string());
    
    if let Some(cmd) = click_cmd_own {
        let btn = Button::builder()
            .child(&label)
            .build();
        btn.set_widget_name("script-btn");
        btn.connect_clicked(move |_| {
            let _ = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        });
        container.append(&btn);
    } else {
        container.append(&label);
    }

    let cmd_own = command.to_string();
    let prefix_own = prefix.to_string();
    let manager = get_manager();
    let key = format!("{}-{}", cmd_own, interval_secs);

    let mut rx = {
        let mut map = manager.lock().unwrap();
        if let Some(worker) = map.get(&key) {
            worker.tx.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(16);
            let worker = ScriptWorker { tx: tx.clone() };
            
            let cmd_for_thread = cmd_own.clone();
            tokio::spawn(async move {
                loop {
                    let output = Command::new("sh")
                        .arg("-c")
                        .arg(&cmd_for_thread)
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

            map.insert(key, worker);
            rx
        }
    };

    let l = label.clone();
    gtk4::glib::MainContext::default().spawn_local(async move {
        // Receive initial value if any (though broadcast doesn't store, so we wait for first)
        while let Ok(out) = rx.recv().await {
            let text = if prefix_own.is_empty() {
                out.trim().to_string()
            } else {
                format!("{} {}", prefix_own, out.trim())
            };
            l.set_label(&text);
        }
    });
}

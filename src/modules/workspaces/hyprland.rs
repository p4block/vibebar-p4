use super::{WorkspaceButtonData, WorkspaceTarget};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone)]
struct Workspace {
    id: i64,
    name: String,
}

pub(super) fn is_available() -> bool {
    socket_path(".socket.sock").is_some_and(|path| path.exists())
}

pub(super) fn run(
    mut rx_ws: tokio::sync::mpsc::UnboundedReceiver<WorkspaceTarget>,
    tx: tokio::sync::mpsc::UnboundedSender<Vec<WorkspaceButtonData>>,
) {
    let tx_actions = tx.clone();
    std::thread::spawn(move || {
        while let Some(target) = rx_ws.blocking_recv() {
            let WorkspaceTarget::HyprlandId(id) = target else {
                continue;
            };
            let _ = send_command(&format!(
                "eval hl.dispatch(hl.dsp.focus({{ workspace = \"{}\" }}))",
                id
            ));
            send_workspaces(&tx_actions);
        }
    });

    send_workspaces(&tx);

    let Some(events_path) = socket_path(".socket2.sock") else {
        poll_workspaces(tx);
        return;
    };

    let stream = match UnixStream::connect(events_path) {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("[workspaces] hyprland event socket failed: {}", e);
            poll_workspaces(tx);
            return;
        }
    };

    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }

        if workspace_event(&line) {
            send_workspaces(&tx);
        }
    }
}

fn poll_workspaces(tx: tokio::sync::mpsc::UnboundedSender<Vec<WorkspaceButtonData>>) {
    loop {
        send_workspaces(&tx);
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn workspace_event(line: &str) -> bool {
    line.starts_with("workspace>>")
        || line.starts_with("focusedmon>>")
        || line.starts_with("createworkspace>>")
        || line.starts_with("destroyworkspace>>")
        || line.starts_with("moveworkspace>>")
        || line.starts_with("renameworkspace>>")
}

fn send_workspaces(tx: &tokio::sync::mpsc::UnboundedSender<Vec<WorkspaceButtonData>>) {
    let Some(workspaces) = read_workspaces() else {
        return;
    };
    let active_id = read_active_workspace_id();

    let ws_data = workspaces
        .into_iter()
        .map(|workspace| WorkspaceButtonData {
            target: WorkspaceTarget::HyprlandId(workspace.id),
            name: workspace.name,
            focused: active_id == Some(workspace.id),
            sort_key: workspace.id,
        })
        .collect();

    let _ = tx.send(ws_data);
}

fn read_workspaces() -> Option<Vec<Workspace>> {
    let json = send_command("j/workspaces").ok()?;
    let val: serde_json::Value = serde_json::from_str(&json).ok()?;

    Some(
        val.as_array()?
            .iter()
            .filter_map(|w| {
                let id = w.get("id")?.as_i64()?;
                let name = w
                    .get("name")
                    .and_then(|name| name.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| id.to_string());
                Some(Workspace { id, name })
            })
            .collect(),
    )
}

fn read_active_workspace_id() -> Option<i64> {
    let json = send_command("j/activeworkspace").ok()?;
    let val: serde_json::Value = serde_json::from_str(&json).ok()?;
    val.get("id")?.as_i64()
}

fn send_command(command: &str) -> Result<String, String> {
    let path = socket_path(".socket.sock").ok_or_else(|| "missing hyprland socket".to_string())?;
    let mut stream = UnixStream::connect(path).map_err(|e| e.to_string())?;
    stream
        .write_all(command.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.shutdown(std::net::Shutdown::Write).ok();

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    Ok(response)
}

fn socket_path(socket_name: &str) -> Option<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    Some(
        PathBuf::from(runtime_dir)
            .join("hypr")
            .join(signature)
            .join(socket_name),
    )
}

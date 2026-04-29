use super::{WorkspaceButtonData, WorkspaceTarget};
use futures::StreamExt;

pub(super) fn run(
    rx_ws: tokio::sync::mpsc::UnboundedReceiver<WorkspaceTarget>,
    tx: tokio::sync::mpsc::UnboundedSender<Vec<WorkspaceButtonData>>,
) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("[workspaces] failed to create sway runtime: {}", e);
            return;
        }
    };

    rt.block_on(async move {
        let events_connection = match swayipc_async::Connection::new().await {
            Ok(connection) => connection,
            Err(e) => {
                eprintln!("[workspaces] sway IPC unavailable: {}", e);
                return;
            }
        };

        let mut events = match events_connection
            .subscribe([swayipc_async::EventType::Workspace])
            .await
        {
            Ok(events) => events,
            Err(e) => {
                eprintln!("[workspaces] sway workspace subscription failed: {}", e);
                return;
            }
        };

        let mut query_connection = match swayipc_async::Connection::new().await {
            Ok(connection) => connection,
            Err(e) => {
                eprintln!("[workspaces] sway query connection failed: {}", e);
                return;
            }
        };

        let mut command_connection = match swayipc_async::Connection::new().await {
            Ok(connection) => connection,
            Err(e) => {
                eprintln!("[workspaces] sway command connection failed: {}", e);
                return;
            }
        };

        tokio::spawn(async move {
            let mut rx_ws = rx_ws;
            while let Some(target) = rx_ws.recv().await {
                let WorkspaceTarget::SwayName(name) = target else {
                    continue;
                };
                let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
                let _ = command_connection
                    .run_command(format!("workspace \"{}\"", escaped))
                    .await;
            }
        });

        send_workspaces(&mut query_connection, &tx).await;

        while let Some(event) = events.next().await {
            match event {
                Ok(swayipc_async::Event::Workspace(_)) => {
                    send_workspaces(&mut query_connection, &tx).await;
                }
                Ok(_) => {}
                Err(e) => eprintln!("[workspaces] sway event error: {}", e),
            }
        }
    });
}

async fn send_workspaces(
    connection: &mut swayipc_async::Connection,
    tx: &tokio::sync::mpsc::UnboundedSender<Vec<WorkspaceButtonData>>,
) {
    match connection.get_workspaces().await {
        Ok(mut workspaces) => {
            workspaces.sort_by_key(|w| (w.num, w.name.clone()));
            let ws_data = workspaces
                .into_iter()
                .map(|w| WorkspaceButtonData {
                    target: WorkspaceTarget::SwayName(w.name.clone()),
                    name: w.name,
                    focused: w.focused,
                    sort_key: w.num as i64,
                })
                .collect();
            let _ = tx.send(ws_data);
        }
        Err(e) => eprintln!("[workspaces] sway workspace query failed: {}", e),
    }
}

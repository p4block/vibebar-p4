use gtk4::prelude::*;
use gtk4::Label;
use futures::StreamExt;
use rtnetlink::new_connection;
use netlink_packet_route::address::AddressAttribute;
use tokio::runtime::Runtime;
use glib::{self, MainContext};

pub fn init(container: &gtk4::Box) {
    let label = Label::builder()
        .label("IP: ...")
        .build();

    container.append(&label);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    
    let l = label.clone();
    MainContext::default().spawn_local(async move {
        while let Some(ips) = rx.recv().await {
            l.set_label(&format!("IP: {}", ips));
        }
    });

    // We need a tokio runtime to run the netlink listener
    std::thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let (connection, handle, mut messages) = new_connection().unwrap();
            tokio::spawn(connection);

            // Function to fetch current IPs
            let fetch_ips = |handle: rtnetlink::Handle| async move {
                use futures::TryStreamExt;
                let mut links = handle.address().get().execute();
                let mut ips = Vec::new();
                while let Ok(Some(msg)) = links.try_next().await {
                   for attr in msg.attributes {
                       if let AddressAttribute::Address(addr) = attr {
                           if addr.is_ipv4() {
                               ips.push(addr.to_string());
                           }
                       }
                   }
                }
                ips.join(", ")
            };

            // Initial set and periodic poll
            loop {
                let ips = fetch_ips(handle.clone()).await;
                let _ = tx.send(ips);
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
    });
}

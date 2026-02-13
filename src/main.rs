use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Orientation};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::sync::Arc;

mod modules;

fn create_window(
    app: &Application,
    monitor: &gdk4::Monitor,
    tray_backend: Option<Arc<modules::tray::TrayBackend>>,
) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("vibebar-p4")
        .build();

    // Layer Shell configuration
    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace("vibebar-p4");
    window.set_monitor(monitor);

    // Anchor to bottom, left, and right
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);

    // Window is 800px tall to allow popovers to grow upwards
    window.set_default_size(-1, 800);

    // Reserve 24px of space for the bar (User preference: "keep my css/ultrawide support")
    window.set_exclusive_zone(24);

    let content = gtk4::CenterBox::new();
    content.set_widget_name("main-container");
    content.set_valign(gtk4::Align::End); // Place bar at the bottom
    content.set_height_request(24); // Match exclusive zone height

    let left = Box::new(Orientation::Horizontal, 0);
    let center = Box::new(Orientation::Horizontal, 0);
    let right = Box::new(Orientation::Horizontal, 0);

    // SizeGroup ensures left and right take equal width (Ultrawide support)
    let size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
    size_group.add_widget(&left);
    size_group.add_widget(&right);

    left.set_halign(gtk4::Align::Start);
    center.set_halign(gtk4::Align::Center);
    right.set_halign(gtk4::Align::End);

    content.set_start_widget(Some(&left));
    content.set_center_widget(Some(&center));
    content.set_end_widget(Some(&right));

    window.set_child(Some(&content));

    // Set input region to only the bottom 24px to allow clicks to pass through above the bar
    window.connect_realize(|w| {
        if let Some(surface) = w.surface() {
            let rect = cairo::RectangleInt::new(0, 800 - 24, 10000, 24);
            let region = cairo::Region::create_rectangle(&rect);
            surface.set_input_region(&region);
        }
    });

    // Initialize modules - Left (User Layout)
    modules::disk::init(&left, "/", " ");
    modules::disk::init(&left, "/mnt/storage", " ");
    modules::ram::init(&left);
    modules::gpu::init(&left);
    modules::cpu::init(&left);

    // Initialize modules - Center
    // Use friend's monitor-aware signature if possible, or fallback
    // Friend's signature: init(&Box, Option<String>)
    modules::workspaces::init(&center, monitor.connector().map(|s| s.to_string()));

    modules::mpris::init(&right);
    modules::scripts::init(&right, "checkupdates | wc -l", 3600, "", None);
    modules::scripts::init(
        &right,
        "~/.config/waybar/scripts/airqualityindex.sh",
        1800,
        "",
        None,
    );

    modules::network::init(&right);

    modules::volume::init(&right);
    modules::clock::init(&right);

    if let Some(backend) = tray_backend {
        modules::tray::init(&right, backend);
    }

    window.present();
}

fn main() {
    let app = Application::builder()
        .application_id("com.github.hal.vibebar-p4")
        .build();

    app.connect_activate(|app| {
        // Load CSS once
        let provider = gtk4::CssProvider::new();
        // Load the user's restored style.css
        provider.load_from_data(include_str!("style.css"));

        let tray_backend = if let Ok(rt) = tokio::runtime::Runtime::new() {
            let rt = std::boxed::Box::leak(std::boxed::Box::new(rt));
            std::mem::forget(rt.enter());
            rt.block_on(async { modules::tray::TrayBackend::new().await })
        } else {
            None
        };

        if let Some(display) = gdk4::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            // Create window for each monitor
            let monitors = display.monitors();
            for i in 0..monitors.n_items() {
                if let Some(monitor) = monitors
                    .item(i)
                    .and_then(|m| m.downcast::<gdk4::Monitor>().ok())
                {
                    create_window(app, &monitor, tray_backend.clone());
                }
            }

            // Handle monitor changes
            monitors.connect_items_changed(move |_, _, _, _| {
                let _ = std::process::Command::new("killall")
                    .arg("-SIGUSR2")
                    .arg("vibebar-p4")
                    .spawn();
            });
        }

        // Handle SIGUSR2 for restart
        // 12 is SIGUSR2 on Linux
        glib::unix_signal_add_local(12, move || {
            let exe = std::env::current_exe().unwrap();
            let args: Vec<_> = std::env::args_os().collect();

            // Prepare CStrings for execv
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            let path_c = CString::new(exe.as_os_str().as_bytes()).unwrap();
            let args_c: Vec<CString> = args
                .iter()
                .map(|arg| CString::new(arg.as_bytes()).unwrap())
                .collect();

            let _ = nix::unistd::execv(&path_c, &args_c);

            glib::ControlFlow::Break
        });
    });

    app.run();
}

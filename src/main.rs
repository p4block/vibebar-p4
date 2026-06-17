use gtk4::gio::ApplicationFlags;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Orientation};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::sync::Arc;

mod modules;

const BAR_HEIGHT: i32 = 24;
const INPUT_REGION_WIDTH: i32 = 10_000;

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

    window.set_default_size(-1, BAR_HEIGHT);

    window.set_exclusive_zone(BAR_HEIGHT);

    let content = gtk4::CenterBox::new();
    content.set_widget_name("main-container");
    content.set_height_request(BAR_HEIGHT);

    let left = Box::new(Orientation::Horizontal, 0);
    let center = Box::new(Orientation::Horizontal, 0);
    let right = Box::new(Orientation::Horizontal, 0);

    // SizeGroup ensures left and right take equal width (Ultrawide support)
    let size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
    size_group.add_widget(&left);
    size_group.add_widget(&right);

    left.set_halign(gtk4::Align::Start);
    center.set_halign(gtk4::Align::Center);
    right.set_halign(gtk4::Align::Fill);

    content.set_start_widget(Some(&left));
    content.set_center_widget(Some(&center));
    content.set_end_widget(Some(&right));

    // Push right modules to the right edge
    let right_spacer = Box::new(Orientation::Horizontal, 0);
    right_spacer.set_hexpand(true);
    right.append(&right_spacer);

    window.set_child(Some(&content));

    // Limit input to the bar itself, even if the compositor allocates extra surface height.
    window.connect_realize(|w| {
        if let Some(surface) = w.surface() {
            let input_y = (w.allocated_height() - BAR_HEIGHT).max(0);
            let rect = cairo::RectangleInt::new(0, input_y, INPUT_REGION_WIDTH, BAR_HEIGHT);
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
    modules::workspaces::init(&center);

    modules::mpris::init(&right);
    modules::scripts::init(&right, "checkupdates | wc -l", 3600, "", None);

    modules::network::init(&right);
    modules::aqi::init(&right);
    modules::battery::init(&right);
    modules::brightness::init(&right);
    modules::power_profile::init(&right);

    modules::mic::init(&right);
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
        .flags(ApplicationFlags::ALLOW_REPLACEMENT | ApplicationFlags::REPLACE)
        .build();

    app.connect_activate(|app| {
        // Load CSS once
        let provider = gtk4::CssProvider::new();
        // Load the user's restored style.css
        provider.load_from_data(include_str!("style.css"));

        static TRAY_RUNTIME: std::sync::OnceLock<Option<tokio::runtime::Runtime>> =
            std::sync::OnceLock::new();
        let tray_backend = TRAY_RUNTIME
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("vibebar-tray")
                    .build()
                    .ok()
            })
            .as_ref()
            .and_then(|rt| rt.block_on(async { modules::tray::TrayBackend::new().await }));

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
        }

        // Handle SIGUSR2 for restart
        glib::unix_signal_add_local(nix::libc::SIGUSR2, move || {
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

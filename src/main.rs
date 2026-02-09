use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Orientation};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gdk4;

mod modules;

fn main() {
    let app = Application::builder()
        .application_id("com.github.hal.vibebar-p4")
        .build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("vibebar-p4")
            .build();

        // Layer Shell configuration
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_namespace("vibebar-p4");

        // Anchor to bottom, left, and right
        window.set_anchor(Edge::Bottom, true);
        window.set_anchor(Edge::Left, true);
        window.set_anchor(Edge::Right, true);
        
        // Window is 800px tall to allow popovers to grow upwards
        window.set_default_size(-1, 800);
        
        // Reserve 32px of space for the bar
        window.set_exclusive_zone(32);

        let content = gtk4::CenterBox::new();
        content.set_widget_name("main-container");
        content.set_valign(gtk4::Align::End); // Place bar at the bottom
        content.set_height_request(32);       // Match exclusive zone height
        
        let left = Box::new(Orientation::Horizontal, 0);
        let center = Box::new(Orientation::Horizontal, 0);
        let right = Box::new(Orientation::Horizontal, 0);

        // SizeGroup ensures left and right take equal width
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

        // Set input region to only the bottom 32px to allow clicks to pass through above the bar
        window.connect_realize(|w| {
            if let Some(surface) = w.surface() {
                let rect = cairo::RectangleInt::new(0, 800 - 32, 10000, 32);
                let region = cairo::Region::create_rectangle(&rect);
                surface.set_input_region(&region);
            }
        });

        // Load CSS
        let provider = gtk4::CssProvider::new();
        provider.load_from_data(include_str!("style.css"));
        if let Some(display) = gdk4::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        // Initialize modules - Left
        modules::battery::init(&left);
        modules::disk::init(&left, "/", "  ");
        modules::ram::init(&left);
        modules::cpu::init(&left);
        modules::weather::init(&left);

        // Initialize modules - Center
        modules::workspaces::init(&center);

        // Initialize modules - Right
        modules::mpris::init(&right);
        modules::power_profiles::init(&right);
        modules::scripts::init(&right, "checkupdates | wc -l", 3600, "  ", Some("footclient sh -c \"echo Updating... && paru -Syu\""));
        modules::network::init(&right, "wlan0");
        modules::volume::init(&right);
        modules::brightness::init(&right);
        modules::clock::init(&right);
        modules::tray::init(&right);

        window.present();

        // Handle SIGUSR2 for restart
        // 12 is SIGUSR2 on Linux
        glib::unix_signal_add_local(12, move || {
            let exe = std::env::current_exe().unwrap();
            let args: Vec<_> = std::env::args_os().collect();
            
            // Prepare CStrings for execv
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;
            
            let path_c = CString::new(exe.as_os_str().as_bytes()).unwrap();
            let args_c: Vec<CString> = args.iter()
                .map(|arg| CString::new(arg.as_bytes()).unwrap())
                .collect();
            
            println!("Restarting vibebar-p4 (SIGUSR2 received)...");
            let _ = nix::unistd::execv(&path_c, &args_c);
            
            glib::ControlFlow::Break
        });
    });

    app.run();
}

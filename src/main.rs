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

        // Anchor to bottom, left, and right for solid full-width bar
        window.set_anchor(Edge::Bottom, true);
        window.set_anchor(Edge::Left, true);
        window.set_anchor(Edge::Right, true);
        
        // Reserve 24px of space (for layout consistency)
        window.set_exclusive_zone(24);

        let content = gtk4::CenterBox::new();
        content.set_widget_name("main-container");
        
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
        modules::disk::init(&left, "/", " ");
        modules::disk::init(&left, "/mnt/storage", " ");
        modules::ram::init(&left);
        modules::cpu::init(&left);

        // Initialize modules - Center
        modules::workspaces::init(&center);

        // Initialize modules - Right
        modules::scripts::init(&right, "checkupdates | wc -l", 3600, "");
        modules::scripts::init(&right, "~/.config/waybar/scripts/airqualityindex.sh", 1800, "");
        modules::mpris::init(&right);
        modules::network::init(&right, "enp4s0");
        modules::volume::init(&right);
        modules::clock::init(&right);
        modules::tray::init(&right);

        window.present();
    });

    app.run();
}

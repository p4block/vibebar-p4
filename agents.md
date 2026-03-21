# vibebar-p4 - Agents Documentation

A Rust/GTK4 status bar for Wayland compositors (Sway, Niri).

## Quick Start

```bash
cargo run --release
```

## Architecture

### Core Loop
- `Application` manages GTK4 lifecycle
- `create_window()` initializes bar per monitor
- `glib::timeout_add_local()` triggers module updates
- `tokio::sync::mpsc::unbounded_channel` passes data to UI
- `SIGUSR2` (12) triggers auto-restart via `execv`

### Layout
```
┌─────────────────────────────────────────────────────────────┐
│  Left (User Layout)           Center           Right        │
│  - disk                       - workspaces       - modules   │
│  - disk                       -                 -            │
│  - ram                        -                 -            │
│  - gpu                        -                 -            │
│  - cpu                        -                 -            │
└─────────────────────────────────────────────────────────────┘
```

## Modules

All modules share this pattern:
1. Create `gtk4::Button`
2. Add CSS class `"btn"`
3. Append to container
4. Setup gestures (click/hover)
5. Spawn background thread
6. Update via tokio channel

### System

#### cpu
`init(&gtk4::Box)`
- Unicode block visualization per core
- GHz frequency, temperature
- Click: opens `btop`
- Interval: 100ms

#### ram
`init(&gtk4::Box)`
- Memory usage percentage
- Icon: ``
- Interval: 5s

#### disk
`init(&gtk4::Box, path: &str, label_prefix: &str)`
- Free space in GB
- Supports multiple partitions
- Interval: 60s

#### gpu
`init(&gtk4::Box)`
- Usage %, frequency, power (W)
- Click: opens `btop`
- Right-click: opens `lact`
- Interval: 2s
- Path: `/sys/class/drm/card1/`

### Network

#### network
`init(&gtk4::Box)`
- Connection type (WiFi/Ethernet)
- WiFi: SSID, signal %, frequency
- Ethernet: IP/CIDR
- Click: opens `nmtui`
- Hover: popover with details (speed, IP, SSID)
- Interval: 1s
- Data source: `/proc/net/route`, `/proc/net/dev`

### Workspaces

#### workspaces
`init(&gtk4::Box, monitor_name: Option<String>)`
- Sway or Niri support
- Monitor-aware (Niri connector name)
- Roman numerals for icons
- Click: focuses workspace
- Interval: Real-time

### Media

#### mpris
`init(&gtk4::Box)`
- Active player detection
- Artist, title, status icon
- Interval: 1s

### Audio

#### volume
`init(&gtk4::Box)`
- PulseAudio integration
- Scroll: +/- 5% per notch
- Click: toggle mute
- Icon: `` / `` (muted)
- Interval: On-change

#### mic
`init(&gtk4::Box)`
- Microphone mute status
- Hidden when not muted
- Click: toggle mute
- Interval: On-change

### Utilities

#### clock
`init(&gtk4::Box)`
- Time: ` Mon DD Mon HH:MM`
- Hover: calendar popover
- Interval: 1s

#### scripts
`init(&gtk4::Box, command: &str, interval_secs: u64, prefix: &str, click_command: Option<&str>)`
- Custom shell commands
- JSON output support
- Example: `checkupdates | wc -l`
- Interval: Configurable

#### aqi
`init(&gtk4::Box)`
- Air Quality Index
- WAQI API (token: `eede2aac49a4b420091a181c837a32f7609022dc`)
- City: Murcia (change via `CITY` const)
- Hover: popover with AQI details
- Interval: 30m

### System Tray

#### tray
`init(&gtk4::Box, backend: Arc<TrayBackend>)`
- StatusNotifierItem support
- XEmbed backend for embedded windows
- Hover: popover menu
- Right-click: context menu
- Click: activate app

#### xembed
`new() -> Option<Arc<XEmbedBackend>>`
- X11 tray manager
- 24x24 embedded window capture
- Click propagation
- Menu dismissal

## Adding New Modules

### Template

```rust
pub fn init(container: &gtk4::Box) {
    let btn = Button::builder().label("ICON ...").build();
    btn.add_css_class("btn");
    container.append(&btn);

    // Click handler
    btn.connect_clicked(|_| {
        // Action
    });

    // Background thread
    std::thread::spawn(move || {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        
        gtk4::glib::MainContext::default().spawn_local(async move {
            while let Some(txt) = rx.recv().await {
                btn.set_label(&txt);
            }
        });

        loop {
            let data = fetch_data();
            let _ = tx.send(data);
            std::thread::sleep(Duration::from_secs(5));
        }
    });
}
```

### Module Registration

Edit `src/main.rs` `create_window()`:

```rust
// Left
modules::my_module::init(&left);

// Center (if monitor-aware)
modules::my_module::init(&center, monitor.connector().map(|s| s.to_string()));

// Right
modules::my_module::init(&right);
```

### Update `src/modules/mod.rs`

```rust
pub mod my_module;
```

## Styling

### CSS Classes

| Class | Description |
|-------|-------------|
| `.btn` | All modules base class |
| `.btn:hover` | Hover state (10% opacity) |
| `.btn.focused` | Active workspace (15% opacity) |
| `popover.standard-popover` | Context menus |
| `.popover-title` | Popover headers (amber) |
| `.popover-info` | Info labels |
| `.popover-stats` | Stats labels |
| `.menu-btn` | Menu items |
| `.xembed` | Tray icons |

### Bar Settings

- Height: 24px
- Background: `rgba(0, 0, 0, 0.9)`
- Margin: `0 1280px` (ultrawide gap)
- Exclusive zone: 24px
- Input region: bottom 24px only

### Font

- Primary: `Terminess Nerd Font`
- Fallback: `Font Awesome 6 Free Solid`, `Font Awesome 6 Brands`
- Size: 12px

## NixOS Integration

```nix
inputs = {
  vibebar-p4.url = "git+https://github.com/p4block/vibebar-p4";
};

outputs = { self, nixpkgs, vibebar-p4, ... }: {
  nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
    specialArgs = { inherit vibebar-p4; };
    modules = [ ./configuration.nix ];
  };
};

# In configuration.nix
environment.systemPackages = [
  vibebar-p4.packages.${pkgs.system}.default
];
```

## Dependencies

### Runtime
- `gtk4`, `gdk4`, `gtk4-layer-shell`
- `dbus`, `libpulseaudio`

### Development
- `cargo`, `git`, `pkgconf`

### Optional (modules)
- `nmtui` (network)
- `lact` (GPU)
- `btop` (CPU)
- `footclient` (terminal)

## Troubleshooting

### Bar not visible
- Check exclusive zone: `zathras` or `swaymsg get_bar`
- Verify input region is correct (24px bottom)

### Modules not updating
- Check interval in background thread
- Verify GTK main loop is running

### Tray missing
- Ensure compositor supports SNI
- Check `xembed` backend logs

### SIGUSR2 restart fails
- Verify `nix::unistd::execv` permissions
- Check `current_exe()` path exists

## License

WTFPL (Do What The Fuck You Want To Public License)
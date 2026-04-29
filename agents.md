# vibebar-p4 - Agents Documentation

Rust/GTK4 status bar for Wayland compositors, currently supporting both niri and sway with compositor-specific tray/workspace compatibility paths.

## Quick Start

```bash
cargo run --release
```

Useful checks:

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Architecture

### Core Loop

- `src/main.rs` owns GTK application lifecycle.
- `create_window()` creates one `gtk4::ApplicationWindow` per monitor.
- Each window is converted to a layer-shell surface via `gtk4-layer-shell`.
- Modules are appended directly into left, center, and right GTK boxes.
- Fast UI updates usually use `glib::timeout_add_local()`.
- Blocking or async sources use a background thread plus `tokio::sync::mpsc::unbounded_channel` back to `glib::MainContext::spawn_local()`.
- `SIGUSR2` (12) restarts the process via `nix::unistd::execv`.

### Runtime

`main()` creates and leaks a Tokio runtime before module initialization. This is intentional in the current architecture: tray/SNI code and async module workers assume a Tokio runtime is already entered.

### Layer-Shell Window

- Layer: `Top`
- Namespace: `vibebar-p4`
- Anchors: bottom, left, right
- Default size: full monitor width by `800px` height
- Visible bar: bottom `24px` of that 800px surface
- Exclusive zone: `24px`
- Input region: only the bottom 24px is interactive; the upper popover area is click-through

The 800px surface exists so GTK popovers can grow upward from the bottom bar. Coordinate conversion for tray/XEmbed code must account for this: the layer window's top is `monitor_y + monitor_height - window_height`, not simply `monitor_y`.

### Layout

```text
left:   disk / disk / ram / gpu / cpu
center: workspaces
right:  spacer / mpris / scripts / network / aqi / mic / volume / clock / tray
```

`gtk4::SizeGroup` keeps left and right boxes the same width for the ultrawide-centered layout. CSS currently adds `padding: 0 1280px` to `#main-container`.

## Modules

### System

#### `cpu`

`init(&gtk4::Box)`

- Displays max CPU frequency, temperature, and per-core unicode load bars.
- Click opens `footclient -e btop`.
- Updates every 2 seconds.
- Temperature path is hard-coded: `/sys/class/hwmon/hwmon2/temp1_input`.

#### `ram`

`init(&gtk4::Box)`

- Displays used memory percentage.
- Updates every 30 seconds.

#### `disk`

`init(&gtk4::Box, path: &str, label_prefix: &str)`

- Displays available space for a mount path using `statvfs`.
- Current registrations: `/` and `/mnt/storage`.
- Updates every 300 seconds.

#### `gpu`

`init(&gtk4::Box)`

- Displays GPU busy percentage, frequency, and power.
- Click opens `lact`.
- Updates every 2 seconds.
- AMD sysfs paths are hard-coded under `/sys/class/drm/card1/device/...` and `hwmon7`.

### Network

#### `network`

`init(&gtk4::Box)`

- Displays active non-virtual default-route interface, IP, optional SSID, and traffic speed.
- Left-click opens `footclient nmtui`.
- Hover shows a GTK popover with connection details.
- Reads `/proc/net/route`, `/proc/net/dev`, `/proc/net/wireless`, `ip`, `iwgetid`, and `iwconfig`.
- Bandwidth updates every 1 second.
- IP/SSID/frequency refresh is gated by `last_external_check` every 60 loop iterations.

### Workspaces

#### `workspaces`

`init(&gtk4::Box)`

- Supports both niri and sway.
- `src/modules/workspaces.rs` owns the GTK UI and dynamic backend selection.
- `src/modules/workspaces/niri.rs` owns niri JSON IPC.
- `src/modules/workspaces/sway.rs` owns sway IPC.
- Tries niri first by connecting to `NIRI_SOCKET` directly with JSON IPC.
- On niri, subscribes to `EventStream`.
- On niri, handles `WorkspacesChanged`, `WorkspaceActivated`, `WorkspaceUrgencyChanged`, and `WindowsChanged`.
- On niri, clicks send `Action::FocusWorkspace` by workspace id.
- If niri IPC is unavailable, falls back to sway via `swayipc-async`.
- On sway, subscribes to workspace events, queries `get_workspaces()`, and focuses workspaces by name with `workspace "<name>"`.
- Workspace icons are fixed mappings for names `"1"` through `"7"`, otherwise a dot icon.

### Media

#### `mpris`

`init(&gtk4::Box)`

- Polls `mpris::PlayerFinder` once per second.
- Shows active player artist/title and playback state icon.
- Truncates display text to roughly 60 characters.

### Audio

#### `volume`

`init(&gtk4::Box)`

- PulseAudio/libpulse subscription for sink/server changes.
- Scroll adjusts default sink volume by 5% per accumulated scroll notch via `pactl`.
- Click toggles default sink mute via `pactl`.

#### `mic`

`init(&gtk4::Box)`

- PulseAudio/libpulse subscription for source/server changes.
- Hidden unless the default source is muted.
- Click toggles default source mute via `pactl`.

### Utilities

#### `clock`

`init(&gtk4::Box)`

- Shows local time as `Mon DD Mon HH:MM`.
- Hover shows a full-year GTK popover calendar.
- Updates every 30 seconds; calendar markup refreshes only while the popover is visible.

#### `scripts`

`init(&gtk4::Box, command: &str, interval_secs: u64, prefix: &str, click_command: Option<&str>)`

- Runs shell commands through `sh -c` on a Tokio runtime in a background thread.
- Optional click command also runs through `sh -c`.
- Initial delay is 5 seconds.
- If stdout parses as JSON, `json["text"]` is used when present.
- Current registration: `checkupdates | wc -l` every 3600 seconds with prefix ``.

#### `aqi`

`init(&gtk4::Box)`

- Fetches WAQI data every 30 minutes.
- Token is hard-coded in `src/modules/aqi.rs`.
- City is hard-coded to `Murcia`; fallback geolocation code exists only if `CITY` is empty.

### System Tray

#### `tray`

`init(&gtk4::Box, backend: Arc<TrayBackend>)`

- Hosts StatusNotifierItem icons through the `system-tray` crate.
- Maintains item and menu state in shared maps.
- Left-click activates an SNI item.
- Right-click shows a GTK popover menu for SNI items.
- Also subscribes to XEmbed events and creates `.btn.xembed` buttons for legacy tray icons.

#### `xembed`

`new() -> Option<Arc<XEmbedBackend>>`

- Manages the legacy XEmbed system tray selection `_NET_SYSTEM_TRAY_S{screen}` using `x11rb`.
- Creates a mapped override-redirect X11 manager window at `-32000,-32000`, 24x24.
- Applies an empty SHAPE bounding/input region while hidden.
- Sets EWMH metadata: window name/class, `_NET_WM_WINDOW_TYPE_DOCK`, skip taskbar/pager, below, PID, opacity 0%.
- Claims the tray selection and sends the `MANAGER` client event.
- Reparents docked client windows under the manager and sends XEmbed embedded/activate/focus events.
- Captures icon pixels once per second with `get_image`.
- Tracks per-window capture failures and removes broken windows after `MAX_FAILS = 3`.

Niri-specific behavior:

- `should_avoid_onscreen_manager()` returns true when `NIRI_SOCKET` is set or `XDG_CURRENT_DESKTOP` contains `niri`.
- On niri, XEmbed icons and left-clicks remain enabled.
- On niri, XEmbed right-click/context menus are intentionally suppressed. Testing showed Xwayland context menus are not globally placeable on niri: they appear only relative to existing Xwayland surfaces and disappear when no Xwayland surface is present on the workspace.
- Set `VIBEBAR_XEMBED_MOVE_MANAGER=1` to force the old teleporting-manager behavior for debugging or sway-like environments.
- Set `VIBEBAR_XEMBED_NEVER_MOVE_MANAGER=1` to force the niri-safe behavior even outside niri.
- Set `VIBEBAR_NO_XEMBED=1` to disable the legacy XEmbed backend entirely.

## Styling

### CSS Classes

| Class | Description |
| --- | --- |
| `.btn` | Base class for module buttons |
| `.btn:hover` | Hover background |
| `.btn.focused` | Active workspace background |
| `popover.standard-popover contents` | Popover body |
| `popover.standard-popover arrow` | Popover arrow |
| `.popover-title` | Popover headers |
| `.popover-info` | Popover details |
| `.popover-stats` | Monospace popover stats |
| `.menu-btn` | SNI menu items |
| `#tray .btn.xembed` | Legacy XEmbed tray buttons |

### Bar Settings

- Visible height: 24px
- Layer surface height: 800px
- Background: `rgba(0, 0, 0, 0.9)`
- Main-container padding: `0 1280px`
- Exclusive zone: 24px
- Input region: bottom 24px only

### Font

- Primary: `Terminess Nerd Font`
- Fallback: `Font Awesome 6 Free Solid`, `Font Awesome 6 Brands`
- Size: 12px

## Adding New Modules

1. Add a module file under `src/modules/`.
2. Export it from `src/modules/mod.rs`.
3. Register it in `create_window()` in `src/main.rs`.
4. Prefer existing update patterns:
   - `glib::timeout_add_local()` for cheap GTK-main-loop polling.
   - background thread + channel for blocking IO.
   - Tokio runtime only when async libraries are required.
5. Add CSS classes using the existing `.btn`/popover/menu system.

Minimal template:

```rust
pub fn init(container: &gtk4::Box) {
    let btn = gtk4::Button::builder().label("ICON ...").build();
    btn.add_css_class("btn");
    container.append(&btn);

    glib::timeout_add_local(std::time::Duration::from_secs(5), move || {
        btn.set_label("ICON value");
        glib::ControlFlow::Continue
    });
}
```

## Dependencies

Runtime/library highlights:

- GTK/layer shell: `gtk4`, `gdk4`, `glib`, `gtk4-layer-shell`, `cairo-rs`
- Async/system: `tokio`, `futures`, `futures-util`, `nix`
- Compositor IPC: `niri-ipc`, `swayipc-async`
- Tray: `system-tray`, `x11rb`
- Audio/media: `libpulse-binding`, `pulsectl-rs`, `mpris`
- System/network/data: `sysinfo`, `reqwest`, `serde`, `serde_json`

External commands used by modules:

- `footclient`
- `btop`
- `lact`
- `nmtui`
- `pactl`
- `ip`
- `iwgetid`
- `iwconfig`
- `checkupdates`

## Troubleshooting

### Bar Not Visible

- Confirm layer-shell support in the compositor.
- Check the 24px exclusive zone.
- Remember the visible bar is at the bottom of an 800px layer surface.

### Modules Not Updating

- Check the module's timer/thread pattern.
- Verify the GTK main loop and leaked Tokio runtime are running.
- For PulseAudio modules, confirm libpulse can connect to the current PipeWire/Pulse server.

### Workspaces Missing

- On niri, confirm `NIRI_SOCKET` is set and points to a live niri IPC socket.
- On sway, confirm `swaymsg -t get_workspaces` works in the same environment.
- If neither compositor IPC path is available, the module logs an IPC connection failure and stays empty.

### Tray Missing

- SNI requires the `system-tray` crate client to connect successfully.
- Legacy XEmbed requires Xwayland and no competing `_NET_SYSTEM_TRAY_S{screen}` owner.
- Use `VIBEBAR_NO_XEMBED=1` to isolate SNI-only behavior.

### XEmbed Context Menus On Niri

- XEmbed right-click menus are intentionally disabled by default on niri.
- They rely on X11 override-redirect popup semantics that do not map cleanly to niri's Xwayland surface model.
- Use `VIBEBAR_XEMBED_MOVE_MANAGER=1` only for debugging; it may reintroduce ghost/placement issues.

### SIGUSR2 Restart Fails

- Verify `current_exe()` points to an executable path.
- Check permissions for `execv`.

## Known Code Review Notes

- Several modules have hard-coded machine-specific paths or commands (`hwmon2`, `card1`, `hwmon7`, `/mnt/storage`, `footclient`, `checkupdates`).
- Full-repo `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are expected to pass.
- `workspaces.rs` keeps separate niri and sway IPC paths; changes should preserve both unless the user explicitly scopes support to one compositor.

## License

WTFPL (Do What The Fuck You Want To Public License)

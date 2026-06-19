# vibebar-p4 - Agents Documentation

Rust/GTK4 status bar for Wayland compositors (niri, Hyprland, sway). Personal-use only; many paths and commands are intentionally hard-coded (see README). This doc is for agents/contributors working on the code.

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

- `src/main.rs` owns the GTK application lifecycle.
- `create_window()` creates one `gtk4::ApplicationWindow` per monitor.
- Each window is converted to a layer-shell surface via `gtk4-layer-shell`.
- Modules are appended directly into left, center, and right GTK boxes.
- Fast UI updates use `glib::timeout_add_local()`.
- Blocking or async sources use a background `std::thread` plus `tokio::sync::mpsc::unbounded_channel` back to `glib::MainContext::default().spawn_local()`.
- `SIGUSR2` (via `glib::unix_signal_add_local`) restarts the process in-place through `nix::unistd::execv`.

### Runtime

`src/runtime.rs` owns a single shared Tokio multi-thread runtime, created lazily via `runtime::init()` (a `OnceLock<Runtime>`) and used for the lifetime of the process. `runtime::handle()` returns a clonable `Handle` that any module can `spawn` onto.

- The tray/SNI backend is constructed with `runtime::init().block_on(TrayBackend::new())`.
- Async modules (`aqi`, `scripts`) call `runtime::handle().spawn(async { ... })` instead of spinning up their own thread + runtime.
- Modules that only need blocking I/O or mainloop polling (PulseAudio `volume`/`mic`, network, brightness inotify watcher) still use plain `std::thread::spawn` since they don't need a Tokio context.

### Layer-Shell Window

- Layer: `Top`
- Namespace: `vibebar-p4`
- Anchors: bottom, left, right
- Default size: full monitor width × `BAR_HEIGHT` (24px)
- Exclusive zone: `24px` (`BAR_HEIGHT`)
- Input region: a single rectangle spanning `INPUT_REGION_WIDTH` (10_000) × `BAR_HEIGHT`, positioned at the bottom of whatever height the compositor allocated (`allocated_height - BAR_HEIGHT`, clamped at 0). This keeps the bar clickable and lets any popover overflow above it remain click-through.

There is no oversized/oversized layer surface. The bar is exactly `24px` tall.

### Layout

```text
left:   disk /  disk /mnt/storage  ram  gpu  cpu
center: workspaces
right:  (hexpand spacer)  mpris  scripts  network  aqi  battery  brightness  power_profile  mic  volume  clock  tray
```

`gtk4::SizeGroup` keeps left and right boxes the same width for the ultrawide-centered layout. The right box starts with an `hexpand` spacer that pushes modules to the right edge. CSS adds `padding: 0 1280px` to `#main-container` for the ultrawide centering.

## Modules

Each module is `pub fn init(container: &gtk4::Box, ...)` and registers widgets + timers. `src/modules/ui.rs` provides shared `.btn` builders and cached label setters to avoid redundant GTK writes.

### System

#### `cpu`

`init(&gtk4::Box)`

- Displays max CPU frequency, temperature, and per-core unicode load bars.
- Click opens `footclient -e btop`.
- Initial update on startup, then every 2 seconds.
- Temperature hwmon is **auto-discovered** via `find_cpu_hwmon()`, matching sensor names `k10temp`, `coretemp`, `zenpower`, `k8temp`, `fam15h_power` that have a `temp1_input`.

#### `ram`

`init(&gtk4::Box)`

- Displays used memory percentage.
- Updates every 30 seconds.

#### `disk`

`init(&gtk4::Box, path: &str, label_prefix: &str)`

- Displays available space for a mount path using `statvfs`.
- Registered: `/` and `/mnt/storage`.
- Updates every 300 seconds.

#### `gpu`

`init(&gtk4::Box)`

- Displays AMD GPU busy %, frequency, power draw, and **VRAM %**.
- Click opens `lact`.
- Initial update on startup, then every 2 seconds.
- DRM device base is **auto-discovered** by `find_gpu_device_base()` (the `cardN` — not `cardN-*` — whose `device/` has `gpu_busy_percent`). VRAM comes from `mem_info_vram_used`/`mem_info_vram_total`. The hwmon used for frequency/power is discovered by `find_hwmon_path()` as the hwmon subdir containing `power1_average` (reads `freq1_input` and `power1_average`).

### Network

#### `network`

`init(&gtk4::Box)`

- Displays active non-virtual default-route interface, IP, optional SSID, and up/down traffic speed.
- Left-click opens `footclient nmtui`.
- Hover shows a GTK popover with connection details (interface, IP, SSID/signal, stats).
- Reads `/proc/net/route`, `/proc/net/dev`, `/proc/net/wireless`, and shells out to `ip` and `iwgetid -r`.
- Virtual interfaces (`veth`, `docker`, `br-*`, `virbr*`, `cni*`, `lxc*`, `tun0`, `tap*`, `lo`, plus `/sys/class/net/<iface>/type == 3`) are filtered out.
- Bandwidth updates every 1 second. IP/SSID refresh is gated by `last_external_check` every 60 loop iterations.

### Workspaces

#### `workspaces`

`init(&gtk4::Box)`

- Backends, tried in order on a dedicated `std::thread`:
  1. **niri** — `src/modules/workspaces/niri.rs`, raw JSON IPC over `NIRI_SOCKET`.
  2. **Hyprland** — `src/modules/workspaces/hyprland.rs`, raw Unix sockets (`.socket.sock` for commands, `.socket2.sock` for events; falls back to polling if no event socket).
  3. **sway** — `src/modules/workspaces/sway.rs`, `swayipc-async`.
- `src/modules/workspaces.rs` owns the GTK UI and a `WorkspaceTarget` enum (`HyprlandId(i64)`, `NiriId(u64)`, `SwayName(String)`) for clicks.
- niri: subscribes to `EventStream`, handles `WorkspacesChanged`, `WorkspaceActivated`, `WorkspaceUrgencyChanged`, `WindowsChanged`; clicks send `Action::FocusWorkspace` by id.
- Hyprland: reads workspace list via `getworkspaces`, listens on the event socket for `workspace>>`/`destroyworkspace>>`/etc.; clicks dispatch `focusWorkspace` by id.
- sway: subscribes to workspace events, queries `get_workspaces()`, focuses by name with `workspace "<name>"`.
- Workspace icons are fixed mappings for names `"1"` through `"7"`, otherwise a dot icon (``).

### Media

#### `mpris`

`init(&gtk4::Box)`

- Polls `mpris::PlayerFinder` once per second.
- Shows active player artist/title and playback-state icon.
- Truncates display text to roughly 60 characters.
- Click runs `playerctl play-pause`.

### Audio

#### `volume`

`init(&gtk4::Box)`

- PulseAudio/libpulse subscription for sink/server changes (reactive display updates via `pipewire-pulse` compatibility).
- Scroll adjusts default sink volume by 5% per accumulated scroll notch via `wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+`/`5%-`.
- Click toggles default sink mute via `wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle`.
- Control commands go through WirePlumber (`wpctl`) rather than the PulseAudio compat shim (`pactl`).

#### `mic`

`init(&gtk4::Box)`

- PulseAudio/libpulse subscription for source/server changes (reactive display updates via `pipewire-pulse` compatibility).
- Hidden unless the default source is muted.
- Click toggles default source mute via `wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle`.

### Laptop

#### `battery`

`init(&gtk4::Box)`

- Scans `/sys/class/power_supply` for `BAT0`/`BAT1` of type `Battery`; aggregates capacity and rolls up status (Charging → Discharging → Full → Unknown).
- Auto-hides when no battery is found.
- Updates every 30 seconds.

#### `brightness`

`init(&gtk4::Box)`

- Reads backlight % directly from sysfs (`/sys/class/backlight/<dev>/brightness` and `max_brightness`); no `brightnessctl` call for reads.
- Scroll adjusts ±5% via `brightnessctl set 5%+`/`5%-` (brightnessctl handles the polkit/udev permissions that direct sysfs writes would lack).
- Auto-hides when no backlight device exists.
- **Reactive**: a background thread watches the sysfs `brightness` file with `nix::sys::inotify` (`IN_MODIFY`) and refreshes the label only when the value actually changes (covers hardware keys, brightnessctl, and direct sysfs writes). Falls back to a 5s poll if the kernel/driver does not support inotify on the attribute.

#### `power_profile`

`init(&gtk4::Box)`

- Reads current profile via `powerprofilesctl get`; click cycles to the next profile from `powerprofilesctl list` (falls back to `[power-saver, balanced, performance]`).
- Auto-hides when `powerprofilesctl` is unavailable.
- Updates every 10 seconds.

### Utilities

#### `clock`

`init(&gtk4::Box)`

- Shows local time as `%a %d %b %H:%M` (e.g. `Wed 19 Jun 14:30`).
- Hover shows a full-year calendar in a GTK popover (markup generated from `chrono`).
- Updates every 30 seconds.

#### `scripts`

`init(&gtk4::Box, command: &str, interval_secs: u64, prefix: &str, click_command: Option<&str>)`

- Runs simple commands directly; falls back to `sh -c` only when shell metacharacters (`| & ; < > $ \` " ' ( )`) are present.
- `checkupdates | wc -l` is special-cased to run `checkupdates` directly and count stdout lines in-process.
- Optional click command follows the same direct-or-shell path.
- Initial update on startup.
- If stdout parses as JSON, `json["text"]` is used when present.
- Current registration: `checkupdates | wc -l` every 3600 seconds.

#### `aqi`

`init(&gtk4::Box)`

- Fetches WAQI data every 30 minutes.
- Token and city are hard-coded in `src/modules/aqi.rs` (`Murcia`). A Mozilla geolocation fallback exists only when `CITY` is empty.
- Creates a new `reqwest::Client` per fetch.

### System Tray

#### `tray`

`init(&gtk4::Box, backend: Arc<TrayBackend>)`

- Hosts StatusNotifierItem icons through the `system-tray` crate on the shared tray Tokio runtime.
- Maintains item and menu state in shared maps.
- Left-click activates an SNI item.
- Right-click shows a GTK popover menu for SNI items.
- Also subscribes to XEmbed events and creates `.btn.xembed` buttons for legacy tray icons.

#### `xembed`

`new() -> Option<Arc<XEmbedBackend>>`

- Manages the legacy XEmbed system tray selection `_NET_SYSTEM_TRAY_S{screen}` using `x11rb`.
- Creates a mapped override-redirect X11 manager window at `(-32000, -32000)`, 24×24.
- Applies an empty SHAPE bounding/input region while hidden.
- Sets EWMH metadata: window name/class, `_NET_WM_WINDOW_TYPE_DOCK`, skip taskbar/pager, below, PID, opacity 0%.
- Claims the tray selection and sends the `MANAGER` client event.
- Reparents docked client windows under the manager and sends XEmbed embedded/activate/focus events.
- Captures icon pixels once per second with `get_image`.
- Tracks per-window capture failures and removes broken windows after `MAX_FAILS = 3`.

Niri-specific behavior:

- `should_avoid_onscreen_manager()` returns true when `NIRI_SOCKET` is set or `XDG_CURRENT_DESKTOP` contains `niri`.
- On niri, XEmbed icons and left-clicks remain enabled.
- On niri, XEmbed right-click/context menus are intentionally suppressed. Xwayland context menus are not globally placeable on niri: they appear only relative to existing Xwayland surfaces and disappear when no Xwayland surface is present on the workspace.
- `VIBEBAR_XEMBED_MOVE_MANAGER=1` — force the old teleporting-manager behavior for debugging or sway-like environments.
- `VIBEBAR_XEMBED_NEVER_MOVE_MANAGER=1` — force the niri-safe behavior even outside niri.
- `VIBEBAR_NO_XEMBED=1` — disable the legacy XEmbed backend entirely.

## Styling

### CSS Classes

| Class | Description |
| --- | --- |
| `*` (reset) | Font family (`Terminess Nerd Font` + Font Awesome fallbacks), 12px size, no shadows |
| `#main-container` | Bar background `rgba(0,0,0,0.9)`, ultrawide padding `0 1280px` |
| `.btn` | Base class for module buttons (`all: unset`, `padding: 0 10px`, `min-height: 24px`) |
| `.btn label` | Inner label padding |
| `.btn:hover` | Hover background |
| `.btn.focused` | Active workspace background |
| `popover.standard-popover contents` / `arrow` | Popover body and arrow |
| `.popover-title` / `.popover-info` / `.popover-stats` | Popover text styles |
| `.menu-btn` (incl. `:hover`/`:active`/`:disabled`/`label`) | SNI menu items |
| `separator` | Thin divider in menus |
| `#tray .btn.xembed` (+ `image`) | Legacy XEmbed tray buttons |

### Bar Settings

- Visible height: 24px
- Layer surface height: 24px
- Background: `rgba(0, 0, 0, 0.9)`
- Main-container padding: `0 1280px`
- Exclusive zone: 24px
- Input region: bottom 24px (clipped dynamically to allocated height)

### Font

- Primary: `Terminess Nerd Font`
- Fallback: `Font Awesome 6 Free Solid`, `Font Awesome 6 Brands`
- Size: 12px

## Adding New Modules

1. Add a module file under `src/modules/`.
2. Export it from `src/modules/mod.rs`.
3. Register it in `create_window()` in `src/main.rs`.
4. Use `src/modules/ui.rs` for standard `.btn` buttons and cached label updates.
5. Prefer existing update patterns:
   - `glib::timeout_add_local()` for cheap GTK-main-loop polling.
   - background thread + channel for blocking IO.
   - `crate::runtime::handle().spawn(async { ... })` for async libraries (reqwest, mpris, system-tray). Do **not** create per-module `tokio::runtime::Runtime`s.
6. Add CSS classes using the existing `.btn`/popover/menu system.

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

Runtime/library highlights (see `Cargo.toml`):

- GTK/layer shell: `gtk4`, `gdk4`, `glib`, `gtk4-layer-shell`, `cairo-rs`
- Async/system: `tokio`, `futures`, `futures-util`, `nix` (features: `fs`, `process`, `inotify`), `async-trait`
- Compositor IPC: `niri-ipc`, `swayipc-async`
- Tray: `system-tray`, `x11rb` (`all-extensions`)
- Audio/media: `libpulse-binding`, `mpris`
- System/network/data: `sysinfo`, `reqwest`, `serde`, `serde_json`, `chrono`

External commands used by modules:

- `footclient`, `btop`, `lact`, `nmtui` (clicks)
- `wpctl` (volume/mic control), `playerctl` (media)
- `brightnessctl`, `powerprofilesctl` (laptop)
- `ip`, `iwgetid` (network)
- `checkupdates` (scripts)

Note: `volume` and `mic` control commands go through `wpctl` (WirePlumber), while reactive display updates use libpulse's subscription API (which talks to PipeWire via the `pipewire-pulse` compat server).

## Troubleshooting

### Bar Not Visible

- Confirm layer-shell support in the compositor.
- Check the 24px exclusive zone.

### Modules Not Updating

- Check the module's timer/thread pattern.
- Verify the GTK main loop is running.
- For PulseAudio modules, confirm libpulse can connect to the current PipeWire/Pulse server.

### Workspaces Missing

- niri: confirm `NIRI_SOCKET` is set and points to a live niri IPC socket.
- Hyprland: confirm `~/.config/hypr/.socket.sock` / `.socket2.sock` exist.
- sway: confirm `swaymsg -t get_workspaces` works in the same environment.
- Backends are tried niri → Hyprland → sway; the first that connects wins.

### Tray Missing

- SNI requires the `system-tray` crate client to connect successfully (the tray runtime is created lazily on `activate`).
- Legacy XEmbed requires Xwayland and no competing `_NET_SYSTEM_TRAY_S{screen}` owner.
- Use `VIBEBAR_NO_XEMBED=1` to isolate SNI-only behavior.

### XEmbed Context Menus On Niri

- XEmbed right-click menus are intentionally disabled by default on niri.
- They rely on X11 override-redirect popup semantics that do not map cleanly to niri's Xwayland surface model.
- Use `VIBEBAR_XEMBED_MOVE_MANAGER=1` only for debugging; it may reintroduce ghost/placement issues.

### SIGUSR2 Restart Fails

- The handler `unwrap()`s `current_exe()` and the `CString` conversions; a failure here will panic in the signal handler. Verify `current_exe()` points to a valid executable path.

## Known Code Review Notes

- `aqi.rs` has a hard-coded WAQI token and city.
- Several modules hard-code machine-specific mounts/commands (`/mnt/storage`, `footclient`, `checkupdates`, `BAT0`/`BAT1`).
- Full-repo `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are expected to pass.
- `workspaces.rs` keeps separate niri/Hyprland/sway IPC paths; changes should preserve all three unless explicitly scoped down.

## License

WTFPL (Do What The Fuck You Want To Public License)

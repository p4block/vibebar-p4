# vibebar-p4

⚠️ **Purely vibe-coded. Personal status bar for my own machine. Not intended for anyone else.** ⚠️

A custom Wayland status bar built with Rust and GTK4. It runs on my machine, against my compositors, my monitors, my mounts, and my external tools. The `flake.nix` and `PKGBUILD` exist for my own installs; they are not a promise of portability. Anything listed under [Hard-coded](#hard-coded--machine-specific) below is intentional and will not be made configurable.

## Stack

- Rust 2024 edition, GTK4 + `gtk4-layer-shell` (Wayland layer-shell protocol)
- Compositor IPC: `niri-ipc`, `swayipc-async`, raw Unix sockets (Hyprland)
- Audio: `libpulse-binding`
- Media: `mpris` (D-Bus)
- System tray: `system-tray` (SNI host) + `x11rb` (XEmbed fallback for legacy icons)
- System info: `sysinfo`, `nix` (`statvfs`), direct sysfs reads
- Async: one shared `tokio` runtime (`src/runtime.rs`) for the tray/SNI backend and async modules (`aqi`, `scripts`); plain background threads for mainloop/polling modules (PulseAudio `volume`/`mic`, network, brightness inotify watcher)
- Full list in [`Cargo.toml`](Cargo.toml)

## Modules

```text
left:   disk /  disk /mnt/storage  ram  gpu  cpu
center: workspaces
right:  mpris  scripts  network  aqi  battery  brightness  power_profile  mic  volume  clock  tray
```

| Module | What it does |
| --- | --- |
| `cpu` | Max frequency, temperature, per-core unicode load bars (▂▃▄▅▆▇█). Temperature hwmon is auto-discovered (`k10temp`/`coretemp`/`zenpower`/`k8temp`/`fam15h_power`). Click → `footclient -e btop`. 2s. |
| `ram` | Used memory %. 30s. |
| `disk` | Available space for a mount via `statvfs`. Registered for `/` and `/mnt/storage`. 300s. |
| `gpu` | AMD GPU busy %, frequency, power draw, and VRAM %. DRM card and hwmon are auto-discovered. Click → `lact`. 2s. |
| `network` | Default-route interface, IP, optional SSID, live up/down traffic with auto unit scaling. Hover → popover with details. Click → `footclient nmtui`. Traffic 1s, IP/SSID every 60 ticks. |
| `workspaces` | Auto-detects **niri → Hyprland → sway**. Clickable workspace buttons with fixed icon mapping for names `1`–`7`. |
| `mpris` | Active player artist/title + playback-state icon. Click → `playerctl play-pause`. Truncates to ~60 chars. 1s. |
| `volume` | Default sink volume %, scroll adjusts ±5% via `wpctl`, click toggles mute. Reactive via libpulse subscription. |
| `mic` | Default source; hidden unless muted. Click toggles mute via `wpctl`. Reactive via libpulse subscription. |
| `battery` | Aggregated `BAT0`/`BAT1` capacity with charging icon. Auto-hides when no battery is present. 30s. |
| `brightness` | Backlight %. Scroll adjusts ±5% via `brightnessctl`. Reads directly from sysfs; updates reactively via inotify on the brightness file. Auto-hides when no backlight device exists. |
| `power_profile` | Cycles `powerprofilesctl` profiles (`power-saver` → `balanced` → `performance`) on click. 10s. |
| `clock` | Local time `Wed 19 Jun HH:MM`. Hover → full-year calendar popover. 30s. |
| `scripts` | Runs a command at an interval with an optional prefix and click command. Simple commands run directly; shell metacharacters fall back to `sh -c`. JSON stdout supporting `{"text": ...}`. `checkupdates \| wc -l` special-cased. |
| `aqi` | WAQI air-quality fetch for a hard-coded city/token. 30 min. |
| `tray` | StatusNotifierItem host plus a legacy XEmbed backend. |

## Layer Shell

- Layer `Top`, namespace `vibebar-p4`, anchored bottom/left/right
- Surface height and exclusive zone: `24px`
- Input region is clipped to the bar height so any popover overflow above the bar stays click-through

## Running

```bash
cargo run --release
```

Send `SIGUSR2` to the running process to restart in-place via `execv`.

## NixOS

`flake.nix` exposes `packages.default` and `nixosModules.default`. The module (`programs.vibebar-p4.enable`) wraps `brightnessctl` and `power-profiles-daemon` into `PATH`, installs the `brightnessctl` udev rules, enables `services.power-profiles-daemon` (toggle with `enablePowerProfilesDaemon`), and ships a `systemd.user.services.vibebar-p4` unit.

```nix
inputs.vibebar-p4.url = "github:p4block/vibebar-p4";
# ...
modules = [
  vibebar-p4.nixosModules.default
  { programs.vibebar-p4.enable = true; }
];
```

An Arch `PKGBUILD` (`vibebar-p4-git`) is also included.

## Hard-coded / machine-specific

Intentional, not bugs:

- `aqi.rs`: WAQI token + city `Murcia`
- `main.rs`: disk mounts `/` and `/mnt/storage`
- `style.css`: `#main-container` `padding: 0 1280px` (tuned for my ultrawide)
- `battery.rs`: only looks at `BAT0`/`BAT1`
- External commands assumed on `PATH`: `footclient`, `btop`, `lact`, `nmtui`, `wpctl`, `playerctl`, `ip`, `iwgetid`, `checkupdates`, `brightnessctl`, `powerprofilesctl`

CPU temperature hwmon and GPU DRM/hwmon paths are **auto-discovered**, not hard-coded.

## License

WTFPL (Do What The Fuck You Want To Public License) — see [LICENSE](LICENSE).

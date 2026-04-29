# Repo Review Notes

Date: 2026-04-29

## Summary

The repository builds, but it is a personal-machine-oriented bar with several hard-coded paths, commands, and compositor assumptions. Workspace support covers both niri and sway. The largest current compatibility decision is the niri/XEmbed compromise: legacy XEmbed tray icons can be shown and left-clicked, but XEmbed right-click menus are disabled on niri because Xwayland popup placement is unreliable.

## Verified

```bash
cargo check
```

Result: passes.

## Formatting

`rustfmt --check --edition 2024 src/modules/xembed.rs src/modules/tray.rs` passes after the recent tray work.

Full-repo formatting is not clean. Existing modules such as `network.rs`, `mpris.rs`, `clock.rs`, and `disk.rs` contain rustfmt drift. Treat repo-wide formatting as a separate cleanup to avoid mixing mechanical churn with behavior changes.

## Current Architecture

- `main.rs` creates a GTK application and one layer-shell window per monitor.
- The layer-shell window is bottom anchored, full width, and 800px tall.
- The visible bar is the bottom 24px of that window.
- The input region is clipped to the bottom 24px so the popover area remains click-through.
- A global Tokio runtime is created and leaked before module initialization.
- Modules either poll on the GTK main loop, spawn blocking threads, or use a background Tokio runtime/thread and send data back to GTK.

## Findings

### High: XEmbed Right-Click Menus Are Not Reliable On Niri

XEmbed right-click menus are intentionally suppressed on niri. Attempts to move or shape the manager window did not make X11 override-redirect menus appear globally. Menus depended on existing Xwayland windows and were offset relative to them.

Impact: legacy tray icons on niri can display and left-click, but their context menus are unavailable.

Recommended follow-up: prefer SNI-native applications where menu support matters.

### Medium: Workspace Support Has Two IPC Paths

`src/modules/workspaces.rs` now supports both compositors through split backends. The top-level module owns the GTK UI and backend selection, `src/modules/workspaces/niri.rs` handles `NIRI_SOCKET` raw JSON IPC, and `src/modules/workspaces/sway.rs` handles sway through `swayipc-async`.

Impact: future workspace changes can regress one compositor while testing only the other.

Recommended follow-up: preserve both click target forms: niri focuses by workspace id, sway focuses by workspace name.

### Medium: Hard-Coded Machine Paths

Several modules assume this host's device layout:

- `cpu.rs`: `/sys/class/hwmon/hwmon2/temp1_input`
- `gpu.rs`: `/sys/class/drm/card1/device/...` and `hwmon7`
- `main.rs`: disk module includes `/mnt/storage`
- `style.css`: `#main-container` has `padding: 0 1280px`

Impact: behavior will degrade or display zeros on other machines.

Recommended follow-up: move these into a small config layer or environment-variable overrides.

### Medium: Command Assumptions

Modules spawn external commands without availability checks:

- `footclient`
- `btop`
- `lact`
- `nmtui`
- `pactl`
- `ip`
- `iwgetid`
- `iwconfig`
- `checkupdates`

Impact: missing commands silently fail or leave modules degraded.

Recommended follow-up: centralize command spawning and log failures consistently.

### Medium: GPU Click Handler Is Ambiguous

`gpu.rs` has `btn.connect_clicked()` opening `lact`. It also creates a left-click `GestureClick` intended to open `btop`, but the controller is not added to the button.

Impact: docs that say left-click opens `btop` are wrong; reliable current behavior is `lact`.

Recommended follow-up: decide on desired click behavior and either add the gesture controller or remove the dead gesture block.

### Medium: `scripts.rs` Ignores `click_command`

The public function accepts `click_command: Option<&str>`, but the argument is named `_click_command` and not used.

Impact: docs or callers may assume click support exists when it does not.

Recommended follow-up: implement click command handling or remove the parameter.

### Low: Network Module Has An Unused Container

`network.rs` creates `module_box` and appends it to the container, but the actual button is appended directly to the same container instead of inside `module_box`.

Impact: likely harmless extra empty widget.

Recommended follow-up: remove `module_box` or append the button to it.

### Low: AQI Token Is Hard-Coded

`aqi.rs` includes a WAQI token and city constant in source.

Impact: personal token/config cannot be rotated or changed without recompilation.

Recommended follow-up: read token/city from environment variables or a config file.

## Documentation Updates Made

- Rewrote `agents.md` to match the current code.
- Rewrote `docs/ghost-window-investigation.md` as a resolved compatibility note.
- Added this repo review note.
- Restored and documented sway workspace support after the initial review caught the mismatch.

## Suggested Next Cleanup Order

1. Configurize machine-specific paths and commands.
2. Fix or remove dead click parameters/gestures.
3. Add a lightweight manual test checklist for niri and sway sessions.
4. Run repo-wide rustfmt in a dedicated formatting-only commit.

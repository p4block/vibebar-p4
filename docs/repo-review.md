# Repo Review Notes

Date: 2026-04-29

## Summary

The repository builds, but it is a personal-machine-oriented bar with several hard-coded paths, commands, and compositor assumptions. Workspace support covers both niri and sway. The largest current compatibility decision is the niri/XEmbed compromise: legacy XEmbed tray icons can be shown and left-clicked, but XEmbed right-click menus are disabled on niri because Xwayland popup placement is unreliable.

## Verified

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Result: all pass after the cleanup pass.

## Formatting

Full-repo formatting is clean.

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

Decision: this is intentional for this machine. Configurability is not a current goal.

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

### Resolved: GPU Click Handler Was Ambiguous

`gpu.rs` now has a single click path: click opens `lact`. The unused `GestureClick` path that attempted to open `btop` was removed.

### Resolved: `scripts.rs` Ignored `click_command`

`scripts.rs` now wires `click_command` to a left-click `sh -c` spawn when one is provided.

### Resolved: Network Module Had An Unused Container

`network.rs` now appends its button into `module_box` instead of appending the box and button as siblings.

### Resolved: Dead Uncompiled Module Files

Removed stale unexported files `src/modules/icon_label.rs` and `src/modules/ip.rs`.

### Low: AQI Token Is Hard-Coded

`aqi.rs` includes a WAQI token and city constant in source.

Decision: this is intentional for this machine. Configurability is not a current goal.

## Documentation Updates Made

- Rewrote `agents.md` to match the current code.
- Rewrote `docs/ghost-window-investigation.md` as a resolved compatibility note.
- Added this repo review note.
- Restored and documented sway workspace support after the initial review caught the mismatch.
- Updated this review after the cleanup pass resolved formatting, clippy, dead module files, the GPU click ambiguity, `scripts` click command handling, and the network container issue.

## Suggested Next Cleanup Order

1. Centralize command spawning/logging if silent failures become annoying.
2. Add a lightweight manual test checklist for niri and sway sessions.

# Ghost Window Investigation - vibebar-p4 / niri

## Status

Resolved as a compatibility compromise.

Legacy XEmbed tray icons are kept enabled on niri, but XEmbed right-click/context menus are disabled by default on niri because they cannot be made reliable with niri's Xwayland surface model.

## Original Symptom

After switching from sway to niri, vibebar could create a large or confusing "ghost" Xwayland window. Closing that ghost could cause vibebar to spin at high CPU.

The issue was localized to the legacy XEmbed tray compatibility path in `src/modules/xembed.rs`.

## Current Behavior

### SNI Tray

StatusNotifierItem icons and GTK popover menus remain supported through `src/modules/tray.rs`.

- Left-click activates the SNI item.
- Right-click opens a GTK `Popover` menu when menu data exists.
- Menus are rendered by vibebar/GTK, not by Xwayland.

### XEmbed Tray

Legacy XEmbed icons remain supported:

- vibebar claims `_NET_SYSTEM_TRAY_S{screen}`.
- XEmbed clients dock under a hidden manager window.
- Icon pixels are captured with X11 `get_image`.
- Left-clicks are forwarded to the docked X11 window.

On niri:

- XEmbed right-click actions return early and do not try to open X11 context menus.
- This avoids random menu placement and ghost-window behavior.

Outside niri:

- The old manager-window movement path remains available for sway-like environments.

## Why XEmbed Menus Are Disabled On Niri

XEmbed context menus are X11 override-redirect popups. In testing under niri/Xwayland:

- Menus appeared only when an Xwayland surface existed on the current workspace.
- If a large Xwayland window covered the desktop, placement could look correct.
- If a smaller Xwayland window existed, menus appeared inside or relative to that window at a large offset.
- If only native Wayland windows existed, the menu did not appear.

This means the X11 popup was not being placed in a compositor-global coordinate space. Attempts to temporarily move or shape the XEmbed manager window did not make the behavior reliable.

## Current XEmbed Manager Design

`src/modules/xembed.rs` creates a mapped X11 manager window:

- Size: `24x24`
- Hidden position: `-32000,-32000`
- `override_redirect(1)`
- 32-bit visual when available
- `_NET_WM_WINDOW_OPACITY = 0`
- Empty SHAPE bounding/input region while hidden
- EWMH metadata:
  - `WM_NAME = vibebar-p4-xembed-tray`
  - `WM_CLASS = vibebar-p4-xembed-tray\0vibebar-p4\0`
  - `_NET_WM_NAME = vibebar-p4 XEmbed tray`
  - `_NET_WM_WINDOW_TYPE_DOCK`
  - `_NET_WM_STATE_SKIP_TASKBAR`
  - `_NET_WM_STATE_SKIP_PAGER`
  - `_NET_WM_STATE_BELOW`
  - `_NET_WM_PID`

The manager must remain mapped because docked XEmbed clients expect a mapped tray owner.

## Environment Switches

### Disable XEmbed Entirely

```bash
VIBEBAR_NO_XEMBED=1 vibebar-p4
```

Use this to isolate SNI-only tray behavior.

### Force Old Manager Movement

```bash
VIBEBAR_XEMBED_MOVE_MANAGER=1 vibebar-p4
```

Use this only for debugging or sway-like environments. On niri it may reintroduce ghost or random-placement behavior.

### Force Niri-Safe Mode

```bash
VIBEBAR_XEMBED_NEVER_MOVE_MANAGER=1 vibebar-p4
```

Forces no manager movement even outside niri.

## Detection

Niri-safe mode is enabled when either condition is true:

- `NIRI_SOCKET` is set.
- `XDG_CURRENT_DESKTOP` contains `niri` case-insensitively.

`VIBEBAR_XEMBED_MOVE_MANAGER=1` overrides this and forces the old behavior.

## Debugging Commands

### Check Niri Windows

```bash
niri msg windows
niri msg focused-window
```

### Check X11 Windows

```bash
xwininfo -root -tree
```

Look for `vibebar-p4-xembed-tray`.

### Check Tray Selection Owner

```bash
xprop -root _NET_SYSTEM_TRAY_S0
```

### Monitor CPU

```bash
htop -p "$(pgrep vibebar-p4 || pgrep vibebar)"
```

## Files Of Interest

| File | Purpose |
| --- | --- |
| `src/main.rs` | Layer-shell window setup and module registration |
| `src/modules/tray.rs` | SNI tray host and GTK tray UI |
| `src/modules/xembed.rs` | Legacy XEmbed tray manager |
| `src/style.css` | Tray and popover styling |
| `agents.md` | Current architecture and maintenance notes |

## Follow-Up Options

- Keep current compromise: XEmbed icons plus left-clicks on niri, no XEmbed right-click menus.
- Add a visible fallback GTK menu for known XEmbed clients only if a reliable source of actions can be found. XEmbed itself does not expose a standard menu model equivalent to SNI.
- Remove XEmbed support entirely on niri by default if the hidden manager causes future compositor regressions.

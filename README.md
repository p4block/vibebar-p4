# vibebar-p4

⚠️ **WARNING: This application is purely vibe-coded for personal use.** ⚠️

`vibebar-p4` is a custom status bar for Wayland compositors (like Sway or Niri) built with Rust and GTK4. It is designed to replace Waybar with a more performant implementation.

## Implementation Details

- **Core**: Built with Rust using `gtk4` + `gtk4-layer-shell` (Wayland layer-shell protocol).
- **Async Execution**: `tokio` for background data fetching; `gtk4::glib::MainContext` for UI updates.
- **Dependencies**: See [`Cargo.toml`](Cargo.toml) for the full crate list. Key highlights:
  - **IPC**: `swayipc-async` (Sway), `niri-ipc` (Niri), raw Unix sockets (Hyprland)
  - **Audio**: `libpulse-binding` (PulseAudio sink/source volume & mute)
  - **Media**: `mpris` (MPRIS D-Bus media player integration)
  - **System Tray**: `system-tray` (SNI host) + `x11rb` (XEmbed fallback for legacy icons)
  - **System Info**: `sysinfo` (CPU load/freq, memory), `nix` (statvfs for disk)
  - **HTTP**: `reqwest` (AQI API fetching)
  - **D-Bus**: `zbus` (transitive via system-tray and mpris)
- **Modules**:
    - **CPU**: Internal implementation using unicode blocks (▂▃▄▅▆▇█) for real-time load visualization.
    - **Memory/Disk**: System monitoring using `sysinfo` and `nix::sys::statvfs`.
    - **Network**: `/proc/net` bandwidth monitoring with automatic unit scaling; popover with IP/SSID/signal details.
    - **Workspaces**: Auto-detects Niri → Hyprland → Sway; clickable workspace buttons.
    - **Media**: MPRIS integration for music control and status.
    - **Volume**: PulseAudio integration with scroll-to-adjust and click-to-mute.
    - **Laptop Power**: Battery, brightness control (`brightnessctl`), and `powerprofilesctl` profile switching.
    - **System Tray**: StatusNotifierItem (SNI) host + XEmbed backend for legacy icons.

## Design Aesthetic
The bar follows a minimal custom aesthetic with specific horizontal margins and a solid background to match my previous `waybar` configuration.

## Running
```bash
cargo run --release
```

## NixOS / Sway Integration

To use `vibebar-p4` in your NixOS configuration, add it to your flake inputs:

```nix
inputs = {
  vibebar-p4.url = "git+https://github.com/p4block/vibebar-p4"; # Or local path
};

outputs = { self, nixpkgs, vibebar-p4, ... }: {
  nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
    specialArgs = { inherit vibebar-p4; };
    modules = [
      ./configuration.nix
    ];
  };
};
```

Then add it to your packages (e.g., in `configuration.nix` or Home Manager):

```nix
environment.systemPackages = [
  vibebar-p4.packages.${pkgs.system}.default
];
```

Or enable the provided NixOS module:

```nix
{
  inputs.vibebar-p4.url = "github:p4block/vibebar-p4";

  outputs = { nixpkgs, vibebar-p4, ... }: {
    nixosConfigurations.your-laptop = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        vibebar-p4.nixosModules.default
        {
          programs.vibebar-p4.enable = true;
        }
      ];
    };
  };
}
```

The package wrapper includes `brightnessctl` and `powerprofilesctl` in `PATH`. The NixOS module also installs the `brightnessctl` udev rules and enables `services.power-profiles-daemon` by default.

### In your Sway config:
Replace `waybar` with `vibebar-p4`:

```sway
bar {
    status_command vibebar-p4
}
# Or simply run it as an exec:
exec vibebar-p4
```

## License
This project is licensed under the **WTFPL (Do What The Fuck You Want To Public License)** - see the [LICENSE](LICENSE) file for details.

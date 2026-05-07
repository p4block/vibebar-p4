# vibebar-p4

⚠️ **WARNING: This application is purely vibe-coded for personal use.** ⚠️

`vibebar-p4` is a custom status bar for Wayland compositors (like Sway or Niri) built with Rust and GTK4. It is designed to replace Waybar with a more performant implementation.

## Implementation Details

- **Core**: Built with Rust using `gtk4` and `gtk4-layer-shell`.
- **Async Execution**: Uses `tokio` for background data fetching (network, stats, etc.) and `gtk4::glib::MainContext` for UI updates.
- **Modules**:
    - **CPU**: Internal implementation using unicode blocks (▂▃▄▅▆▇█) for real-time load visualization.
    - **Memory/Disk**: Efficient system monitoring using `sysinfo` and `/proc` filesystems.
    - **Network**: Mbps/Gbps bandwidth monitoring with automatic unit scaling.
    - **Workspaces**: Roman numeral (I, II, III...) workspace buttons via Sway IPC or Niri IPC.
    - **Media**: MPRIS integration for music control and status.
    - **Volume**: Direct PulseAudio/Pipewire integration.
    - **Laptop Power**: Battery, brightness control, and `powerprofilesctl` profile switching.
    - **System Tray**: StatusNotifierItem (SNI) host implementation for background application icons.

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

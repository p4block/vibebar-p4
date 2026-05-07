{
  description = "A purely vibe-coded status bar for personal use";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustVersion = pkgs.rust-bin.stable.latest.default;

        nativeBuildInputs = with pkgs; [
          pkg-config
          rustVersion
          wrapGAppsHook4
        ];

        buildInputs = with pkgs; [
          gtk4
          gtk4-layer-shell
          dbus
          libpulseaudio
        ];

        runtimeInputs = with pkgs; [
          brightnessctl
          power-profiles-daemon
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "vibebar-p4";
          version = "0.1.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          inherit nativeBuildInputs buildInputs;

          postInstall = ''
            wrapProgram $out/bin/vibebar-p4 \
              --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath buildInputs} \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeInputs}
          '';
        };

        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;
          packages = runtimeInputs;
        };
      }
    ) // {
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.programs.vibebar-p4;
        in
        {
          options.programs.vibebar-p4 = {
            enable = lib.mkEnableOption "vibebar-p4";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.system}.default;
              defaultText = lib.literalExpression "vibebar-p4.packages.\${pkgs.system}.default";
              description = "vibebar-p4 package to run.";
            };

            enablePowerProfilesDaemon = lib.mkOption {
              type = lib.types.bool;
              default = true;
              description = "Enable power-profiles-daemon for the power profile module.";
            };
          };

          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ cfg.package ];
            services.udev.packages = [ pkgs.brightnessctl ];
            services.power-profiles-daemon.enable = lib.mkIf cfg.enablePowerProfilesDaemon true;

            systemd.user.services.vibebar-p4 = {
              Unit = {
                Description = "vibebar-p4 status bar";
                After = [ "graphical-session.target" ];
                PartOf = [ "graphical-session.target" ];
              };

              Service = {
                ExecStart = "${cfg.package}/bin/vibebar-p4";
                Restart = "on-failure";
              };

              Install.WantedBy = [ "graphical-session.target" ];
            };
          };
        };
    };
}

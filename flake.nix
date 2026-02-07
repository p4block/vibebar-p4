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
              --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath buildInputs}
          '';
        };

        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;
        };
      }
    );
}

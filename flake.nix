{
  description = "listbot - Discord Bot";

  inputs = {
    nixpkgs.url      = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        toolchain = pkgs.rust-bin.fromRustupToolchain {
          channel = "nightly-2021-11-01";
          components = [ "clippy" "rustfmt" "rust-src" ];
        };
      in
      {
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            openssl
            pkgconfig
            exa
            fd
            toolchain
            valgrind
            massif-visualizer
          ];

          shellHook = ''
            echo "Loaded devshell"
          '';
        };
      }
    );
}

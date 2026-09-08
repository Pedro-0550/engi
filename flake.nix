{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixos-unstable";
    nixpkgs-tracy.url = "github:NixOS/nixpkgs/pull/428369/head";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-tracy,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit overlays system; };
        pkgs-tracy = import nixpkgs-tracy { inherit overlays system; };
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShell = pkgs.mkShell rec {
          nativeBuildInputs = with pkgs; [ pkg-config ];
          packages =
            with pkgs;
            [
              # Build tools
              gnumake
              cmake
              pkg-config

              rust
              libclang

              stdenv.cc.cc
              tmux

              stdenv

              pkgsStatic.xz.dev
              lldb
              llvm
              zstd
              openssl.dev
              pkgs.llvmPackages.bintools
              git
            ]
            ++ [ pkgs-tracy.tracy ];
          LIBCLANG_PATH = "${pkgs.libclang}/lib";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath packages;
          CONFIG_MINIMAL_LIBC = "y";
          RUST_BACKTRACE = "1";
        };

      }
    );
}

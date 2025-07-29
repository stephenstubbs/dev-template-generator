{
  description = "A Rust CLI tool for generating multi-language nix development environments";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rustfmt"
          ];
        };

        dev-template-generator = pkgs.rustPlatform.buildRustPackage {
          pname = "dev-template-generator";
          version = "0.1.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          buildInputs =
            with pkgs;
            [
              openssl
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ];

          meta = with pkgs.lib; {
            description = "A Rust CLI tool for generating multi-language nix development environments";
            homepage = "https://github.com/stephenstubbs/dev-template-generator";
            license = licenses.mit;
            maintainers = [ ];
          };
        };
      in
      {
        packages = {
          default = dev-template-generator;
          dev-template-generator = dev-template-generator;
        };

        apps = {
          default = {
            type = "app";
            program = "${dev-template-generator}/bin/dev-template-generator";
          };
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain
            openssl
            pkg-config
            cargo-deny
            cargo-edit
            cargo-watch
            rust-analyzer
          ];

          env = {
            RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          };
        };
      }
    );
}

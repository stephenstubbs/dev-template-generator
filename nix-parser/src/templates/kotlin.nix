{
  description = "A Nix-flake-based Kotlin development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      javaVersion = 21; # Change this value to update the whole stack

      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs { inherit system; };
          }
        );
    in
    {
      overlays.default = final: prev: rec {
        jdk = prev."jdk${toString javaVersion}";
        gradle = prev.gradle.override { java = jdk; };
        kotlin = prev.kotlin.override { jre = jdk; };
      };

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              kotlin
              gradle
              gcc
              ncurses
              patchelf
              zlib
            ];
          };
        }
      );
    };
}

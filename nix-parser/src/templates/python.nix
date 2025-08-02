{
  description = "A Nix-flake-based Python development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
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
      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              python311
              python311Packages.pip
              python311Packages.virtualenv
            ];

            shellHook = ''
              # Create virtual environment if it doesn't exist
              if [ ! -d ".venv" ]; then
                python -m venv .venv
              fi

              # Activate virtual environment
              source .venv/bin/activate

              # Upgrade pip in virtual environment
              pip install --upgrade pip
            '';

            venvShellHook = pkgs.python311Packages.venvShellHook;
          };
        }
      );
    };
}

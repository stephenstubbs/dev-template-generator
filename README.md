# Nix Flake Generator

A Rust CLI tool for generating nix development environments. Supports 35 programming languages with intelligent template merging for single-language and polyglot projects.

## Features

- **35 Language Templates**: Support for all major programming languages including Rust, Go, Python, Java, TypeScript, Haskell, and many more
- **Multi-Language Environments**: Combine multiple languages in a single development environment
- **Self-Contained**: All templates are embedded - no external dependencies
- **Intelligent Merging**: Automatically handles overlays, inputs, and package conflicts
- **Nix Flakes**: Generates modern nix flake.nix files for reproducible environments
- **Automatic Formatting**: Generated flake.nix files are automatically formatted with nixfmt when available

## Supported Languages

bun, c-cpp, clojure, csharp, cue, dhall, elixir, elm, gleam, go, hashi, haskell, haxe, java, kotlin, latex, nickel, nim, nix, node, ocaml, opa, php, protobuf, pulumi, python, r, ruby, rust, rust-toolchain, scala, shell, swift, vlang, zig

## Installation

### Option 1: Direct Installation with Nix Profile

```bash
# Install directly from GitHub
nix profile install github:stephenstubbs/nix-flake-generator

# Verify installation
nix-flake-generator --help
```

### Option 2: Run Without Installing

```bash
# Run directly from GitHub
nix run github:stephenstubbs/nix-flake-generator -- --help
```

### Option 3: Add to Your Flake

Add to your `flake.nix` inputs:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    nix-flake-generator = {
      url = "github:stephenstubbs/nix-flake-generator";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, nix-flake-generator, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = [
          nix-flake-generator.packages.${system}.default
        ];
      };
    };
}
```

### Option 4: Development Environment

```bash
# Clone and enter development environment
git clone https://github.com/stephenstubbs/nix-flake-generator
cd nix-flake-generator
nix develop

# Build and run
cargo build --release
./target/release/nix-flake-generator --help
```

## Usage

### Commands

```bash
# List all available templates
nix-flake-generator list

# Initialize a development environment (single or multi-language)
nix-flake-generator init <template(s)> [--path <directory>]

# Show help
nix-flake-generator --help
```

### Examples

#### Single Language Environments

```bash
# Create a Rust development environment
nix-flake-generator init rust

# Create a Python environment in a specific directory
nix-flake-generator init python --path my-python-project

# Create a Go environment
nix-flake-generator init go --path go-service
```

#### Multi-Language Environments

```bash
# Full-stack web development (Rust backend, Node frontend)
nix-flake-generator init rust,node --path fullstack-app

# JVM polyglot environment
nix-flake-generator init java,kotlin,scala --path jvm-project

# Systems programming languages
nix-flake-generator init rust,c-cpp,zig --path systems-project

# Data science stack
nix-flake-generator init python,r --path data-project

# Functional programming environment
nix-flake-generator init haskell,elixir,ocaml --path fp-project
```

#### Using the Generated Environment

After generating a template:

```bash
# Enter the directory
cd my-project

# Activate the nix development shell
nix develop

# Or use direnv (if you have it configured)
echo "use flake" > .envrc
# For nushell users:
echo "use flake" | save .envrc
direnv allow
```

#### Code Formatting

Generated `flake.nix` files are automatically formatted with [nixfmt](https://github.com/NixOS/nixfmt) when available. To install nixfmt:

```bash
# Install nixfmt
nix profile install nixpkgs#nixfmt

# Or add to your system configuration
# Or include in your development shell
```

If nixfmt is not available, files will still be generated successfully but without formatting.

### Advanced Usage

#### Custom Template Combinations

The tool intelligently merges templates, handling:
- **Input deduplication**: Automatically manages nix flake inputs
- **Overlay merging**: Combines language-specific overlays
- **Package consolidation**: Merges package lists without conflicts
- **Environment variables**: Preserves language-specific environment setup

#### Example Multi-Language Output

For `nix-flake-generator init rust,go,node --path web-stack`:

```nix
{
  description = "Multi-language development environment (rust, go, node)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    # ... automatically generated configuration
    {
      devShells = forEachSupportedSystem ({ pkgs }: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            # Rust tools
            rustToolchain
            cargo-deny
            cargo-edit
            cargo-watch
            rust-analyzer
            # Go tools
            go
            gotools
            golangci-lint
            # Node.js tools
            nodejs
            yarn
            nodePackages.pnpm
            # Shared dependencies
            openssl
            pkg-config
          ];

          env = {
            RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
          };
        };
      });
    };
}
```

## Template Features

### Language-Specific Features

- **Rust**: Includes rust-overlay, configurable toolchain, common cargo tools
- **Go**: Version management, common go tools, linting
- **Node.js**: Multiple package managers (npm, yarn, pnpm), node2nix
- **Java/Kotlin/Scala**: JDK version management, Maven, Gradle, SBT
- **Python**: Virtual environment support, pip integration
- **Haskell**: GHC, Cabal, HLS (Haskell Language Server)
- **C/C++**: Clang tools, CMake, debugging tools, package managers

### Cross-Platform Support

All templates support:
- x86_64-linux
- aarch64-linux
- x86_64-darwin (macOS Intel)
- aarch64-darwin (macOS Apple Silicon)

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add templates to `src/embedded_templates.rs` if adding language support
5. Test your changes with `cargo test`
6. Submit a pull request

### Adding New Languages

To add support for a new language:

1. Create template files in the `src/templates/` directory
2. Add the template to `src/embedded_templates.rs`
3. Follow the existing template structure
4. Test single and multi-language combinations
5. Update the README with the new language

## License

MIT License - see LICENSE file for details.

## Acknowledgments

This tool was inspired by and builds upon the excellent work in the nix community for creating reproducible development environments. Special thanks to the maintainers of nixpkgs and the various language-specific nix overlays.

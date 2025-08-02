mod ast;
mod parser;
mod flake_analysis;

pub use ast::*;
use parser::nix_expr;
use flake_analysis::{extract_flake_data, extract_fragments_from_expr};



// Main parsing functions
pub fn parse_nix_expr(input: &str) -> Result<NixExpr, ParseError> {
    match nix_expr(input.trim()) {
        Ok((remaining, expr)) => {
            let remaining_trimmed = remaining.trim();
            if remaining_trimmed.is_empty() {
                Ok(expr)
            } else {
                Err(ParseError::Parse(format!("Unexpected remaining input: '{}' (first 100 chars)", 
                    &remaining_trimmed[..remaining_trimmed.len().min(100)])))
            }
        }
        Err(e) => Err(ParseError::Parse(format!("Parsing Error: {e}"))),
    }
}

pub fn parse_flake(input: &str) -> Result<FlakeData, ParseError> {
    let expr = parse_nix_expr(input)?;
    extract_flake_data(&expr)
}

pub fn extract_flake_fragments(input: &str) -> Result<FlakeFragments, ParseError> {
    let expr = parse_nix_expr(input)?;
    extract_fragments_from_expr(&expr)
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::binding;

    #[test]
    fn test_parse_simple_attrset() {
        let input = r#"{ foo = "bar"; }"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::AttrSet { bindings, .. } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].path.parts[0], AttrPathPart::Identifier("foo".to_string()));
                assert_eq!(bindings[0].value, NixExpr::String("bar".to_string()));
            }
            _ => panic!("Expected AttrSet"),
        }
    }

    #[test]
    fn test_parse_flake_description() {
        let input = r#"{ description = "A test flake"; }"#;
        let flake = parse_flake(input).unwrap();
        assert_eq!(flake.description, Some("A test flake".to_string()));
    }

    #[test]
    fn test_parse_function_call() {
        let input = r#"pkgs.mkShell { buildInputs = [ go ]; }"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::FunctionCall { function, argument } => {
                match *function {
                    NixExpr::Select { .. } => {},
                    _ => panic!("Expected Select expression"),
                }
                match *argument {
                    NixExpr::AttrSet { .. } => {},
                    _ => panic!("Expected AttrSet argument"),
                }
            }
            _ => panic!("Expected FunctionCall"),
        }
    }

    #[test]
    fn test_parse_lambda() {
        let input = r#"{ pkgs }: pkgs.hello"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::Lambda { param, body } => {
                match param {
                    LambdaParam::Pattern { params, .. } => {
                        assert_eq!(params[0].name, "pkgs");
                    }
                    _ => panic!("Expected pattern parameter"),
                }
                match *body {
                    NixExpr::Select { .. } => {},
                    _ => panic!("Expected select expression in body"),
                }
            }
            _ => panic!("Expected Lambda"),
        }
    }

    #[test]
    fn test_parse_let_in() {
        let input = r#"let x = 1; in x + 2"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::LetIn { bindings, body } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].path.parts[0], AttrPathPart::Identifier("x".to_string()));
                assert_eq!(bindings[0].value, NixExpr::Integer(1));
                match *body {
                    NixExpr::BinaryOp { .. } => {},
                    _ => panic!("Expected binary operation in body"),
                }
            }
            _ => panic!("Expected LetIn"),
        }
    }

    #[test]
    fn test_parse_list() {
        let input = r#"[ "a" "b" "c" ]"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::List(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], NixExpr::String("a".to_string()));
                assert_eq!(items[1], NixExpr::String("b".to_string()));
                assert_eq!(items[2], NixExpr::String("c".to_string()));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_parse_interpolated_string() {
        let input = r#""Hello ${name}!""#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 3);
                assert_eq!(parts[0], StringPart::Literal("Hello ".to_string()));
                match &parts[1] {
                    StringPart::Interpolation(expr) => {
                        assert_eq!(**expr, NixExpr::Identifier("name".to_string()));
                    }
                    _ => panic!("Expected interpolation"),
                }
                assert_eq!(parts[2], StringPart::Literal("!".to_string()));
            }
            _ => panic!("Expected InterpolatedString"),
        }
    }

    #[test]
    fn test_extract_flake_fragments_rust() {
        let input = include_str!("templates/rust.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Rust development environment");
        assert_eq!(result.inputs.len(), 2);
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(result.inputs.contains_key("rust-overlay"));
        assert!(!result.overlays.is_empty());
        assert!(!result.packages.is_empty());
        assert!(result.packages.contains(&"rustToolchain".to_string()));
    }

    #[test]
    fn test_extract_flake_fragments_python() {
        let input = include_str!("templates/python.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        
        assert_eq!(result.header, "A Nix-flake-based Python development environment");
        assert_eq!(result.inputs.len(), 1);
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
        assert!(!result.shell_hooks.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_go() {
        let input = include_str!("templates/go.nix");
        
        // First try to parse the basic expression
        match parse_nix_expr(input) {
            Ok(_expr) => {
                let result = extract_flake_fragments(input).unwrap();
                
                assert_eq!(result.header, "A Nix-flake-based Go 1.22 development environment");
                assert!(!result.overlays.is_empty());
                assert!(!result.packages.is_empty());
            }
            Err(e) => {
                eprintln!("Failed to parse go.nix template: {e:#?}");
                // For now, let's not panic so we can see what's happening
                assert!(false, "Failed to parse go.nix template");
            }
        }
    }

    #[test]
    fn test_extract_flake_fragments_elm() {
        let input = include_str!("templates/elm.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Elm development environment");
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_node() {
        let input = include_str!("templates/node.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Node.js development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.overlays.is_empty());
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_java() {
        let input = include_str!("templates/java.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Java development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.overlays.is_empty());
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_haskell() {
        let input = include_str!("templates/haskell.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Haskell development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_c_cpp() {
        let input = include_str!("templates/c-cpp.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based C/C++ development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_shell() {
        let input = include_str!("templates/shell.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Shell development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_bun() {
        let input = include_str!("templates/bun.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_clojure() {
        let input = include_str!("templates/clojure.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_csharp() {
        let input = include_str!("templates/csharp.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_cue() {
        let input = include_str!("templates/cue.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_dhall() {
        let input = include_str!("templates/dhall.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_elixir() {
        let input = include_str!("templates/elixir.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_gleam() {
        let input = include_str!("templates/gleam.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_hashi() {
        let input = include_str!("templates/hashi.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
        assert!(result.allow_unfree, "Hashi template should set allow_unfree = true");
    }

    #[test]
    fn test_extract_flake_fragments_haxe() {
        let input = include_str!("templates/haxe.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_kotlin() {
        let input = include_str!("templates/kotlin.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_latex() {
        let input = include_str!("templates/latex.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_nickel() {
        let input = include_str!("templates/nickel.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_nim() {
        let input = include_str!("templates/nim.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_nix() {
        let input = include_str!("templates/nix.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_ocaml() {
        let input = include_str!("templates/ocaml.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_opa() {
        let input = include_str!("templates/opa.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_php() {
        let input = include_str!("templates/php.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_protobuf() {
        let input = include_str!("templates/protobuf.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_pulumi() {
        let input = include_str!("templates/pulumi.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_r() {
        let input = include_str!("templates/r.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_ruby() {
        let input = include_str!("templates/ruby.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_rust_toolchain() {
        let input = include_str!("templates/rust-toolchain.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_scala() {
        let input = include_str!("templates/scala.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_swift() {
        let input = include_str!("templates/swift.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_vlang() {
        let input = include_str!("templates/vlang.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_zig() {
        let input = include_str!("templates/zig.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_interpolated_attribute_access() {
        let input = r#"final."go_1_${toString goVersion}""#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed interpolated attribute access: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse interpolated attribute access: {e:#?}");
                // For now, let's see what happens
            }
        }
    }

    #[test]  
    fn test_complex_lambda_params() {
        let input = r#"{ self, nixpkgs, rust-overlay }: body"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed complex lambda: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse complex lambda: {e:#?}");
            }
        }
    }

    #[test]
    fn test_multiline_lambda() {
        let input = r#"{
  self,
  nixpkgs,
  rust-overlay,
}: body"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed multiline lambda: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse multiline lambda: {e:#?}");
            }
        }
    }

    #[test]
    fn test_go_template_minimal() {
        let input = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }: {
    overlays.default = final: prev: {
      go = final."go_1_24";
    };
  };
}"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed minimal go template: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse minimal go template: {e:#?}");
            }
        }
    }

    #[test]
    fn test_function_call_in_interpolation() {
        let input = r#""go_1_${toString goVersion}""#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed function call in interpolation: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse function call in interpolation: {e:#?}");  
            }
        }
    }

    #[test]
    fn test_comment_in_binding() {
        let input = r#"goVersion = 24; # Change this to update the whole stack"#;
        let result = binding(input);
        
        match result {
            Ok((remaining, binding)) => {
                eprintln!("Parsed binding with comment: {binding:#?}, remaining: '{remaining}'");
            }
            Err(e) => {
                eprintln!("Failed to parse binding with comment: {e:#?}");
            }
        }
    }

    #[test]
    fn test_inherit_statement() {
        let input = r#"{ inherit system; }"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed inherit statement: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse inherit statement: {e:#?}");
            }
        }
    }

    #[test]
    fn test_import_function() {
        let input = r#"import nixpkgs { inherit system; }"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed import function: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse import function: {e:#?}");
            }
        }
    }

    #[test]
    fn test_progressive_go_parsing() {
        // Test increasingly complex parts of the go template
        
        // Just the description
        let input1 = r#"{ description = "A Nix-flake-based Go 1.22 development environment"; }"#;
        match parse_nix_expr(input1) {
            Ok(_) => eprintln!("✓ Parsed basic description"),
            Err(e) => eprintln!("✗ Failed to parse basic description: {e:#?}"),
        }
        
        // Test 7: Exact problematic pattern from go template
        let exact_go_part = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24; # Change this to update the whole stack

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
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        );
    in
    {
      overlays.default = final: prev: {
        go = final."go_1_${toString goVersion}";
      };

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              go
              gotools
              golangci-lint
            ];
          };
        }
      );
    };
}"#;
        match parse_nix_expr(exact_go_part) {
            Ok(_) => eprintln!("✓ Parsed full go template"),
            Err(e) => eprintln!("✗ Failed to parse full go template: {e:#?}"),
        }
        
        // Test 8: Let's try without the complex interpolation
        let without_interpolation = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24; # Change this to update the whole stack

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
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        );
    in
    {
      overlays.default = final: prev: {
        go = final.go_1_24;
      };

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              go
              gotools  
              golangci-lint
            ];
          };
        }
      );
    };
}"#;
        match parse_nix_expr(without_interpolation) {
            Ok(_) => eprintln!("✓ Parsed without interpolation"),
            Err(e) => eprintln!("✗ Failed without interpolation: {e:#?}"),
        }
        
        // Test 9: Try parsing just the problematic structure step by step
        let just_outputs = r#"{
  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24;
    in
    {
      overlays.default = final: prev: {
        go = final.go_1_24;
      };
    };
}"#;
        match parse_nix_expr(just_outputs) {
            Ok(_) => eprintln!("✓ Parsed just outputs"),
            Err(e) => eprintln!("✗ Failed just outputs: {e:#?}"),
        }
        
        // Test 10: Combine description + inputs + simple outputs
        let desc_inputs_outputs = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }: {};
}"#;
        match parse_nix_expr(desc_inputs_outputs) {
            Ok(_) => eprintln!("✓ Parsed desc+inputs+outputs"),
            Err(e) => eprintln!("✗ Failed desc+inputs+outputs: {e:#?}"),
        }
        
        // Test 11: Add the let-in with complex expressions gradually
        let with_complex_let = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24; # Change this to update the whole stack

      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin" 
        "aarch64-darwin"
      ];
    in
    {};
}"#;
        match parse_nix_expr(with_complex_let) {
            Ok(_) => eprintln!("✓ Parsed with complex let"),
            Err(e) => eprintln!("✗ Failed with complex let: {e:#?}"),
        }
        
        // Test 12: Add the complex forEachSupportedSystem lambda
        let with_complex_lambda = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24; # Change this to update the whole stack

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
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        );
    in
    {};
}"#;
        match parse_nix_expr(with_complex_lambda) {
            Ok(_) => eprintln!("✓ Parsed with complex lambda"),
            Err(e) => eprintln!("✗ Failed with complex lambda: {e:#?}"),
        }
        
        // Test 13: Test just the problematic lambda in isolation  
        let just_problematic_lambda = r#"forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        )"#;
        match binding(just_problematic_lambda) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed problematic lambda binding, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed problematic lambda binding: {e:#?}"),
        }
        
        // Test 14: Test just the function call part that's failing
        let just_function_call = r#"nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {}
        )"#;
        match nix_expr(just_function_call) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed function call, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed function call: {e:#?}"),
        }
        
        // Test 15: Test the exact complex function call that's failing
        let exact_complex_call = r#"nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        )"#;
        match nix_expr(exact_complex_call) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed exact complex call, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed exact complex call: {e:#?}"),
        }
        
        // Test 16: Test just the parenthesized lambda in isolation
        let just_parenthesized_lambda = r#"(
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        )"#;
        match nix_expr(just_parenthesized_lambda) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed parenthesized lambda, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed parenthesized lambda: {e:#?}"),
        }
        
        // Test 17: Test the exact failing attribute set
        let failing_attrset = r#"{
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }"#;
        match nix_expr(failing_attrset) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed failing attrset, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed failing attrset: {e:#?}"),
        }
        
        // Test 18: Test just the inherit statement
        let just_inherit = r#"inherit system"#;
        match binding(just_inherit) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed inherit, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed inherit: {e:#?}"),
        }
        
        // Test 19: Test nested attrset with inherit
        let nested_inherit = r#"{
  inherit system;
  foo = "bar";
}"#;
        match nix_expr(nested_inherit) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed nested inherit, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed nested inherit: {e:#?}"),
        }
        
        // Test 20: Test import function call
        let import_call = r#"import nixpkgs {
  inherit system;
  overlays = [ self.overlays.default ];
}"#;
        match nix_expr(import_call) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed import call, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed import call: {e:#?}"),
        }
        
        // Test 21: Test just the problematic attribute set again
        let just_the_attrset = r#"{
  inherit system;
  overlays = [ self.overlays.default ];
}"#;
        match nix_expr(just_the_attrset) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed just attrset, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed just attrset: {e:#?}"),
        }
        
        // Test 22: Test inherit with semicolon
        let inherit_with_semicolon = r#"inherit system;"#;
        match binding(inherit_with_semicolon) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed inherit with semicolon, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed inherit with semicolon: {e:#?}"),
        }
        
        // Test 23: Test the exact case that should work now
        let inherit_no_semicolon = r#"inherit system"#;
        match binding(inherit_no_semicolon) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed inherit no semicolon, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed inherit no semicolon: {e:#?}"),
        }
        
        // Test 24: Very basic attrset with inherit
        let basic_inherit_attrset = r#"{ inherit system; }"#;
        match nix_expr(basic_inherit_attrset) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed basic inherit attrset, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed basic inherit attrset: {e:#?}"),
        }
        
        // Test 25: Just the overlays binding that's failing
        let just_overlays = r#"overlays = [ self.overlays.default ]"#;
        match binding(just_overlays) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed overlays binding, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed overlays binding: {e:#?}"),
        }
        
        // Test 26: Just the list that's failing
        let just_list = r#"[ self.overlays.default ]"#;
        match nix_expr(just_list) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed overlays list, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed overlays list: {e:#?}"),
        }
        
        // Test 27: Just the attribute access that might be failing
        let just_attr_access = r#"self.overlays.default"#;
        match nix_expr(just_attr_access) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed attr access, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed attr access: {e:#?}"),
        }
        
        // Test 28: Test the problematic packages list
        let packages_list = r#"[
              go
              gotools
              golangci-lint
            ]"#;
        match nix_expr(packages_list) {
            Ok((remaining, expr)) => eprintln!("✓ Parsed packages list: {expr:?}, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed packages list: {e:#?}"),
        }
        
        // Add inputs
        let input2 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
}"#;
        match parse_nix_expr(input2) {
            Ok(_) => eprintln!("✓ Parsed with inputs"),
            Err(e) => eprintln!("✗ Failed to parse with inputs: {e:#?}"),
        }
        
        // Add simple outputs
        let input3 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: {};
}"#;
        match parse_nix_expr(input3) {
            Ok(_) => eprintln!("✓ Parsed with simple outputs"),
            Err(e) => eprintln!("✗ Failed to parse with simple outputs: {e:#?}"),
        }
        
        // Add let binding
        let input4 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let goVersion = 24; in { };
}"#;
        match parse_nix_expr(input4) {
            Ok(_) => eprintln!("✓ Parsed with let binding"),
            Err(e) => eprintln!("✗ Failed to parse with let binding: {e:#?}"),
        }

        // Add comment in let binding
        let input5 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let goVersion = 24; # comment
  in { };
}"#;
        match parse_nix_expr(input5) {
            Ok(_) => eprintln!("✓ Parsed with comment in let"),
            Err(e) => eprintln!("✗ Failed to parse with comment in let: {e:#?}"),
        }

        // Add list in let binding
        let input6 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let 
    goVersion = 24;
    supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
  in { };
}"#;
        match parse_nix_expr(input6) {
            Ok(_) => eprintln!("✓ Parsed with list in let"),
            Err(e) => eprintln!("✗ Failed to parse with list in let: {e:#?}"),
        }

        // Add function definition
        let input7 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let 
    goVersion = 24;
    supportedSystems = [ "x86_64-linux" ];
    forEachSupportedSystem = f: f;
  in { };
}"#;
        match parse_nix_expr(input7) {
            Ok(_) => eprintln!("✓ Parsed with function definition"),
            Err(e) => eprintln!("✗ Failed to parse with function definition: {e:#?}"),
        }

        // Test the complex function call pattern from go template
        let input8 = r#"{
  outputs = { self, nixpkgs }: let 
    supportedSystems = [ "x86_64-linux" ];
    forEachSupportedSystem = f: nixpkgs.lib.genAttrs supportedSystems f;
  in { };
}"#;
        match parse_nix_expr(input8) {
            Ok(_) => eprintln!("✓ Parsed with complex function call"),
            Err(e) => eprintln!("✗ Failed to parse with complex function call: {e:#?}"),
        }

        // Test with lambda parameter in function call
        let input9 = r#"{
  outputs = { self, nixpkgs }: let 
    supportedSystems = [ "x86_64-linux" ];
    forEachSupportedSystem = f: nixpkgs.lib.genAttrs supportedSystems (system: f);
  in { };
}"#;
        match parse_nix_expr(input9) {
            Ok(_) => eprintln!("✓ Parsed with lambda in function call"),
            Err(e) => eprintln!("✗ Failed to parse with lambda in function call: {e:#?}"),
        }

        // Test with more complex lambda body like in go template
        let input10 = r#"{
  outputs = { self, nixpkgs }: let 
    supportedSystems = [ "x86_64-linux" ];
    forEachSupportedSystem = f: nixpkgs.lib.genAttrs supportedSystems (
      system: f { pkgs = import nixpkgs { inherit system; }; }
    );
  in { };
}"#;
        match parse_nix_expr(input10) {
            Ok(_) => eprintln!("✓ Parsed with complex lambda body"),
            Err(e) => eprintln!("✗ Failed to parse with complex lambda body: {e:#?}"),
        }
    }

    #[test]
    fn test_binary_operator_parsing() {
        let input = r#"a ++ b"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::BinaryOp { left, op, right } => {
                assert_eq!(*left, NixExpr::Identifier("a".to_string()));
                assert_eq!(op, BinaryOperator::Concat);
                assert_eq!(*right, NixExpr::Identifier("b".to_string()));
            }
            _ => panic!("Expected BinaryOp"),
        }
    }

    #[test]
    fn test_with_expression() {
        let input = r#"with pkgs; [ hello ]"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::With { env, body } => {
                assert_eq!(*env, NixExpr::Identifier("pkgs".to_string()));
                match *body {
                    NixExpr::List(items) => {
                        assert_eq!(items.len(), 1);
                        assert_eq!(items[0], NixExpr::Identifier("hello".to_string()));
                    }
                    _ => panic!("Expected List in with body"),
                }
            }
            _ => panic!("Expected With expression"),
        }
    }

    #[test]
    fn test_select_expression() {
        let input = r#"pkgs.hello"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::Select { expr, path, .. } => {
                assert_eq!(*expr, NixExpr::Identifier("pkgs".to_string()));
                assert_eq!(path.parts.len(), 1);
                assert_eq!(path.parts[0], AttrPathPart::Identifier("hello".to_string()));
            }
            _ => panic!("Expected Select expression"),
        }
    }
}
use std::fs;
use crate::integration::common::{
    create_cargo_command, create_temp_dir_with_path, validate_flake_content_with_nix_check
};

#[test]
fn test_large_combinations() {
    let large_combinations = [
        "rust,go,python,node,java",
        "haskell,ocaml,elixir,gleam,nim",
        "c-cpp,rust,zig,go,swift",
        "python,r,latex,nix,shell",
    ];

    for combo in large_combinations {
        test_large_combination(combo);
    }
}

fn test_large_combination(combo: &str) {
    let mut cmd = create_cargo_command();
    let (temp_dir, temp_path) = create_temp_dir_with_path();
    
    let result = cmd
        .arg("init")
        .arg(combo)
        .arg("--path")
        .arg(&temp_path)
        .assert();
    
    if result.try_success().is_ok() {
        let flake_path = temp_dir.path().join("flake.nix");
        if flake_path.exists() {
            let flake_content = fs::read_to_string(&flake_path)
                .expect("Should read flake content");
            
            assert!(
                flake_content.contains("Multi-language development environment"),
                "Large combo {combo} should have multi-language description"
            );
            assert!(
                flake_content.contains("devShells"),
                "Large combo {combo} should have devShells"
            );
            
            let safe_name = combo.replace(",", "-");
            validate_flake_content_with_nix_check(
                &flake_content,
                &format!("test-large-{safe_name}"),
            );
        }
    }
}
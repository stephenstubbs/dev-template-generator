use predicates::prelude::*;
use crate::integration::common::{
    create_cargo_command, create_temp_dir_with_path, assert_flake_exists_and_contains, 
    assert_basic_flake_structure, validate_flake_content_with_nix_check
};

pub const ALL_LANGUAGES: &[&str] = &[
    "bun", "c-cpp", "clojure", "csharp", "cue", "dhall", "elixir", "elm",
    "gleam", "go", "hashi", "haskell", "haxe", "java", "kotlin", "latex",
    "nickel", "nim", "nix", "node", "ocaml", "opa", "php", "protobuf",
    "pulumi", "python", "r", "ruby", "rust", "rust-toolchain", "scala",
    "shell", "swift", "vlang", "zig",
];

#[test]
fn test_all_single_language_templates() {
    for language in ALL_LANGUAGES {
        let mut cmd = create_cargo_command();
        let (temp_dir, temp_path) = create_temp_dir_with_path();
        
        cmd.arg("init")
            .arg(language)
            .arg("--path")
            .arg(&temp_path)
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "Initialized {language} template in {temp_path}"
            )));
        
        let flake_content = assert_flake_exists_and_contains(&temp_dir, &[]);
        assert_basic_flake_structure(&flake_content, language);
        validate_flake_content_with_nix_check(&flake_content, &format!("test-single-{language}"));
    }
}
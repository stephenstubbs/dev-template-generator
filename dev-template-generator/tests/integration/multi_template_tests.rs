use predicates::prelude::*;
use crate::integration::common::{
    create_cargo_command, create_temp_dir_with_path, assert_flake_exists_and_contains, 
    validate_flake_content_with_nix_check
};

#[test]
fn test_rust_go_combination() {
    let mut cmd = create_cargo_command();
    let (temp_dir, temp_path) = create_temp_dir_with_path();
    
    cmd.arg("init")
        .arg("rust,go")
        .arg("--path")
        .arg(&temp_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Initialized multi-language template (rust,go) in {temp_path}"
        )));
    
    let flake_content = assert_flake_exists_and_contains(
        &temp_dir,
        &[
            "Multi-language development environment (rust, go)",
            "rust-overlay",
            "rustToolchain",
            "go",
            "gotools",
        ]
    );
    
    validate_flake_content_with_nix_check(&flake_content, "test-cli-init-multi-rust-go");
}

#[test]
fn test_jvm_languages_combination() {
    let mut cmd = create_cargo_command();
    let (temp_dir, temp_path) = create_temp_dir_with_path();
    
    cmd.arg("init")
        .arg("java,kotlin,scala")
        .arg("--path")
        .arg(&temp_path)
        .assert()
        .success();
    
    let flake_content = assert_flake_exists_and_contains(
        &temp_dir,
        &[
            "Multi-language development environment (java, kotlin, scala)",
            "maven",
            "gradle",
            "kotlin",
            "scala",
        ]
    );
    
    validate_flake_content_with_nix_check(&flake_content, "test-jvm-combination");
}
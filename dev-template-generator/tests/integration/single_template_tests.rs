use predicates::prelude::*;
use crate::integration::common::{
    create_cargo_command, create_temp_dir_with_path, assert_flake_exists_and_contains, 
    validate_flake_content_with_nix_check
};

#[test]
fn test_rust_template() {
    let mut cmd = create_cargo_command();
    let (temp_dir, temp_path) = create_temp_dir_with_path();
    
    cmd.arg("init")
        .arg("rust")
        .arg("--path")
        .arg(&temp_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Initialized rust template in {temp_path}"
        )));
    
    let flake_content = assert_flake_exists_and_contains(
        &temp_dir,
        &["rust-overlay", "rustToolchain", "cargo-watch"]
    );
    
    validate_flake_content_with_nix_check(&flake_content, "test-cli-init-rust");
}

#[test]
fn test_python_template() {
    let mut cmd = create_cargo_command();
    let (temp_dir, temp_path) = create_temp_dir_with_path();
    
    cmd.arg("init")
        .arg("python")
        .arg("--path")
        .arg(&temp_path)
        .assert()
        .success();
    
    let flake_content = assert_flake_exists_and_contains(
        &temp_dir,
        &["python311", "venvShellHook"]
    );
    
    validate_flake_content_with_nix_check(&flake_content, "test-python-template");
}
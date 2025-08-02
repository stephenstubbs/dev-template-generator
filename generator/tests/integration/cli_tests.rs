use predicates::prelude::*;
use crate::integration::common::create_cargo_command;

#[test]
fn test_help_command() {
    let mut cmd = create_cargo_command();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Generate development environments from nix templates",
        ))
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("list"));
}

#[test]
fn test_list_command() {
    let mut cmd = create_cargo_command();
    cmd.arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available templates:"))
        .stdout(predicate::str::contains("rust - "))
        .stdout(predicate::str::contains("go - "))
        .stdout(predicate::str::contains("python - "))
        .stdout(predicate::str::contains("node - "));
}

#[test]
fn test_missing_template_argument() {
    let mut cmd = create_cargo_command();
    cmd.arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}
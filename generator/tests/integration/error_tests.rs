use predicates::prelude::*;
use crate::integration::common::create_cargo_command;

#[test]
fn test_nonexistent_single_template() {
    let mut cmd = create_cargo_command();
    cmd.arg("init")
        .arg("nonexistent-template")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Template 'nonexistent-template' not found",
        ));
}

#[test]
fn test_nonexistent_multi_template() {
    let mut cmd = create_cargo_command();
    cmd.arg("init")
        .arg("rust,nonexistent,go")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Template 'nonexistent' not found"));
}
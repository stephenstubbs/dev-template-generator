use assert_cmd::Command;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::TempDir;

// Test helper functions
pub fn create_cargo_command() -> Command {
    Command::cargo_bin("dev-template-generator").unwrap()
}

pub fn create_temp_dir_with_path() -> (TempDir, String) {
    let temp_dir = TempDir::new().expect("Should create temp directory");
    let temp_path = temp_dir.path().to_string_lossy().to_string();
    (temp_dir, temp_path)
}

pub fn assert_flake_exists_and_contains(temp_dir: &TempDir, expected_contents: &[&str]) -> String {
    let flake_path = temp_dir.path().join("flake.nix");
    assert!(flake_path.exists(), "flake.nix should be created");
    
    let flake_content = fs::read_to_string(&flake_path).expect("Should read flake content");
    
    for content in expected_contents {
        assert!(flake_content.contains(content), "Flake should contain: {}", content);
    }
    
    flake_content
}

pub fn assert_basic_flake_structure(flake_content: &str, test_name: &str) {
    let required_sections = ["description =", "inputs", "outputs", "devShells", "nixpkgs"];
    for section in &required_sections {
        assert!(
            flake_content.contains(section),
            "{} should have {}",
            test_name,
            section
        );
    }
}

pub fn validate_flake_content_with_nix_check(flake_content: &str, test_name: &str) {
    let temp_dir = TempDir::new().expect("Should create temp directory");
    let temp_path = temp_dir.path();
    
    let flake_path = temp_path.join("flake.nix");
    fs::write(&flake_path, flake_content).expect("Should write flake.nix");
    
    create_additional_files_if_needed(flake_content, temp_path);
    run_nix_validation(temp_path, test_name);
}

fn create_additional_files_if_needed(flake_content: &str, temp_path: &std::path::Path) {
    if flake_content.contains("rust-toolchain.toml") {
        let toolchain_content = r#"[toolchain]
channel = "stable"
components = ["rustfmt", "rust-analyzer"]
"#;
        fs::write(temp_path.join("rust-toolchain.toml"), toolchain_content)
            .expect("Should write rust-toolchain.toml");
    }
}

fn run_nix_validation(temp_path: &std::path::Path, test_name: &str) {
    let path_str = temp_path.to_string_lossy();
    println!("🔍 Running nix flake check on temporary directory for {test_name}");
    
    let output = StdCommand::new("nix")
        .args(["flake", "check", "--no-build", &path_str])
        .output();
    
    match output {
        Ok(result) => {
            if result.status.success() {
                println!("✅ Nix validation passed for {test_name}");
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                let stdout = String::from_utf8_lossy(&result.stdout);
                println!("❌ Nix validation failed for {test_name}:");
                println!("STDOUT: {stdout}");
                println!("STDERR: {stderr}");
                panic!("Nix flake validation failed for {test_name}");
            }
        }
        Err(e) => {
            println!("⚠️  Nix not available, skipping validation: {e}");
        }
    }
}
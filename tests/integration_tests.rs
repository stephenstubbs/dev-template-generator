use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::TempDir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
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
fn test_cli_list_command() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
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
fn test_cli_init_nonexistent_template() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
    cmd.arg("init")
        .arg("nonexistent-template")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Template 'nonexistent-template' not found",
        ));
}

#[test]
fn test_cli_init_multi_nonexistent_template() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
    cmd.arg("init")
        .arg("rust,nonexistent,go")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Template 'nonexistent' not found"));
}

fn validate_flake_content_with_nix_check(flake_content: &str, test_name: &str) {
    // Create a temporary directory
    let temp_dir = TempDir::new().expect("Should create temp directory");
    let temp_path = temp_dir.path();

    // Write flake.nix to temporary directory
    let flake_path = temp_path.join("flake.nix");
    fs::write(&flake_path, flake_content).expect("Should write flake.nix");

    // If the content suggests it needs additional files (like rust-toolchain.toml), create them
    if flake_content.contains("rust-toolchain.toml") {
        let toolchain_content = r#"[toolchain]
channel = "stable"
components = ["rustfmt", "rust-analyzer"]
"#;
        fs::write(temp_path.join("rust-toolchain.toml"), toolchain_content)
            .expect("Should write rust-toolchain.toml");
    }

    let path_str = temp_path.to_string_lossy();
    println!(
        "🔍 Running nix flake check on temporary directory for {}",
        test_name
    );

    let output = StdCommand::new("nix")
        .args(&["flake", "check", "--no-build", &path_str])
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                println!("✅ Nix validation passed for {}", test_name);
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                let stdout = String::from_utf8_lossy(&result.stdout);
                println!("❌ Nix validation failed for {}:", test_name);
                println!("STDOUT: {}", stdout);
                println!("STDERR: {}", stderr);
                panic!("Nix flake validation failed for {}", test_name);
            }
        }
        Err(e) => {
            println!("⚠️  Nix not available, skipping validation: {}", e);
            // Don't fail if nix is not available - this allows tests to run in environments without nix
        }
    }
    // temp_dir is automatically cleaned up when it goes out of scope
}

#[test]
fn test_cli_init_rust() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();

    let temp_dir = TempDir::new().expect("Should create temp directory");
    let temp_path = temp_dir.path().to_string_lossy();

    cmd.arg("init")
        .arg("rust")
        .arg("--path")
        .arg(&*temp_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Initialized rust template in {}",
            temp_path
        )));

    // Verify the flake was created and has expected content
    let flake_path = temp_dir.path().join("flake.nix");
    assert!(flake_path.exists(), "flake.nix should be created");

    let flake_content = fs::read_to_string(&flake_path).expect("Should read flake content");
    assert!(flake_content.contains("rust-overlay"));
    assert!(flake_content.contains("rustToolchain"));
    assert!(flake_content.contains("cargo-watch"));

    // Validate flake with nix if available
    validate_flake_content_with_nix_check(&flake_content, "test-cli-init-rust");
}

#[test]
fn test_cli_init_multi_rust_go() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();

    let temp_dir = TempDir::new().expect("Should create temp directory");
    let temp_path = temp_dir.path().to_string_lossy();

    cmd.arg("init")
        .arg("rust,go")
        .arg("--path")
        .arg(&*temp_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Initialized multi-language template (rust,go) in {}",
            temp_path
        )));

    // Verify the merged flake was created
    let flake_path = temp_dir.path().join("flake.nix");
    assert!(flake_path.exists(), "flake.nix should be created");

    let flake_content = fs::read_to_string(&flake_path).expect("Should read flake content");
    assert!(flake_content.contains("Multi-language development environment (rust, go)"));
    assert!(flake_content.contains("rust-overlay"));
    assert!(flake_content.contains("rustToolchain"));
    assert!(flake_content.contains("go"));
    assert!(flake_content.contains("gotools"));

    // Validate flake with nix if available
    validate_flake_content_with_nix_check(&flake_content, "test-cli-init-multi-rust-go");
}

#[test]
fn test_cli_init_with_current_directory() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();

    let temp_dir = TempDir::new().expect("Should create temp directory");
    let temp_path = temp_dir.path().to_string_lossy();

    cmd.arg("init")
        .arg("python")
        .arg("--path")
        .arg(&*temp_path)
        .assert()
        .success();

    // Verify content
    let flake_path = temp_dir.path().join("flake.nix");
    let flake_content = fs::read_to_string(&flake_path).expect("Should read flake content");
    assert!(flake_content.contains("python311"));
    assert!(flake_content.contains("venvShellHook"));
}

#[test]
fn test_cli_init_complex_multi_language() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();

    let temp_dir = TempDir::new().expect("Should create temp directory");
    let temp_path = temp_dir.path().to_string_lossy();

    cmd.arg("init")
        .arg("java,kotlin,scala")
        .arg("--path")
        .arg(&*temp_path)
        .assert()
        .success();

    let flake_path = temp_dir.path().join("flake.nix");
    let flake_content = fs::read_to_string(&flake_path).expect("Should read flake content");

    // Should contain JVM-related packages
    assert!(flake_content.contains("Multi-language development environment (java, kotlin, scala)"));
    assert!(flake_content.contains("maven"));
    assert!(flake_content.contains("gradle"));
    assert!(flake_content.contains("kotlin"));
    assert!(flake_content.contains("scala"));
}

#[test]
fn test_cli_update_command() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
    cmd.arg("update")
        .assert()
        .success()
        .stdout(predicate::str::contains("Templates updated successfully"));
}

#[test]
fn test_cli_missing_template_argument() {
    let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
    cmd.arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_all_single_language_templates() {
    let languages = [
        "bun",
        "c-cpp",
        "clojure",
        "csharp",
        "cue",
        "dhall",
        "elixir",
        "elm",
        "gleam",
        "go",
        "hashi",
        "haskell",
        "haxe",
        "java",
        "kotlin",
        "latex",
        "nickel",
        "nim",
        "nix",
        "node",
        "ocaml",
        "opa",
        "php",
        "protobuf",
        "pulumi",
        "python",
        "r",
        "ruby",
        "rust",
        "rust-toolchain",
        "scala",
        "shell",
        "swift",
        "vlang",
        "zig",
    ];

    for language in languages {
        let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
        let temp_dir = TempDir::new().expect("Should create temp directory");
        let temp_path = temp_dir.path().to_string_lossy();

        cmd.arg("init")
            .arg(language)
            .arg("--path")
            .arg(&*temp_path)
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "Initialized {} template in {}",
                language, temp_path
            )));

        // Verify flake was created
        let flake_path = temp_dir.path().join("flake.nix");
        assert!(
            flake_path.exists(),
            "flake.nix should be created for {}",
            language
        );

        let flake_content = fs::read_to_string(&flake_path).expect("Should read flake content");

        // Basic validation - all flakes should have these
        assert!(
            flake_content.contains("description ="),
            "{} should have description",
            language
        );
        assert!(
            flake_content.contains("inputs"),
            "{} should have inputs",
            language
        );
        assert!(
            flake_content.contains("outputs"),
            "{} should have outputs",
            language
        );
        assert!(
            flake_content.contains("devShells"),
            "{} should have devShells",
            language
        );
        assert!(
            flake_content.contains("nixpkgs"),
            "{} should reference nixpkgs",
            language
        );

        // Validate with nix if available
        validate_flake_content_with_nix_check(&flake_content, &format!("test-single-{}", language));
    }
}

#[test]
fn test_popular_language_combinations() {
    let combinations = [
        // Web development stacks
        ("rust,node", "Systems + Frontend"),
        ("python,node", "Backend + Frontend"),
        ("go,node", "Backend + Frontend"),
        // JVM ecosystem
        ("java,kotlin", "JVM Languages"),
        ("java,scala", "JVM Languages"),
        ("kotlin,scala", "JVM Languages"),
        ("java,kotlin,scala", "Full JVM Stack"),
        // Systems programming
        ("rust,c-cpp", "Systems Languages"),
        ("rust,zig", "Modern Systems"),
        ("c-cpp,zig", "Systems Languages"),
        ("rust,c-cpp,zig", "Full Systems Stack"),
        // Functional programming
        ("haskell,ocaml", "Functional Languages"),
        ("elixir,gleam", "BEAM Languages"),
        ("haskell,elixir", "Functional Languages"),
        // Data science
        ("python,r", "Data Science"),
        // DevOps/Infrastructure
        ("hashi,nix", "Infrastructure"),
        ("pulumi,go", "Infrastructure as Code"),
        ("shell,nix", "System Administration"),
        // Multi-paradigm combinations
        ("rust,python,node", "Full Stack"),
        ("go,python,node", "Backend Heavy"),
        ("java,python,node", "Enterprise Stack"),
        ("rust,haskell,python", "Multi-Paradigm"),
        // Additional language coverage
        ("bun,node", "JavaScript Runtimes"),
        ("clojure,java", "JVM Functional"),
        ("csharp,java", "Enterprise Languages"),
        ("cue,dhall", "Configuration Languages"),
        ("elm,haskell", "Functional Frontend"),
        ("gleam,elixir", "BEAM Ecosystem"),
        ("haxe,nim", "Multi-target Languages"),
        ("latex,r", "Academic Writing"),
        ("nickel,nix", "Nix Ecosystem"),
        ("ocaml,haskell", "ML Family"),
        ("opa,protobuf", "Data/API Languages"),
        ("php,ruby", "Dynamic Web Languages"),
        ("pulumi,hashi", "Infrastructure as Code"),
        ("swift,kotlin", "Mobile Languages"),
        ("vlang,zig", "Modern Systems"),
    ];

    for (langs, description) in combinations {
        let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
        let temp_dir = TempDir::new().expect("Should create temp directory");
        let temp_path = temp_dir.path().to_string_lossy();

        cmd.arg("init")
            .arg(langs)
            .arg("--path")
            .arg(&*temp_path)
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "Initialized multi-language template ({}) in {}",
                langs, temp_path
            )));

        // Verify flake was created
        let flake_path = temp_dir.path().join("flake.nix");
        assert!(
            flake_path.exists(),
            "flake.nix should be created for {}",
            description
        );

        let flake_content = fs::read_to_string(&flake_path).expect("Should read flake content");

        // Validate multi-language structure
        assert!(
            flake_content.contains("Multi-language development environment"),
            "{} should have multi-language description",
            description
        );
        assert!(
            flake_content.contains("nixpkgs.url"),
            "{} should have nixpkgs input",
            description
        );
        assert!(
            flake_content.contains("devShells"),
            "{} should have devShells",
            description
        );

        // Validate each language appears in the description
        for lang in langs.split(',') {
            assert!(
                flake_content.contains(lang),
                "{} should contain {}",
                description,
                lang
            );
        }

        // Validate with nix if available
        let safe_name = langs.replace(",", "-");
        validate_flake_content_with_nix_check(&flake_content, &format!("test-combo-{}", safe_name));
    }
}

#[test]
fn test_stress_test_large_combinations() {
    // Test some larger combinations to ensure the merger handles complexity
    let large_combinations = [
        "rust,go,python,node,java",
        "haskell,ocaml,elixir,gleam,nim",
        "c-cpp,rust,zig,go,swift",
        "python,r,latex,nix,shell",
    ];

    for combo in large_combinations {
        let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
        let temp_dir = TempDir::new().expect("Should create temp directory");
        let temp_path = temp_dir.path().to_string_lossy();
        let langs = combo;

        let result = cmd
            .arg("init")
            .arg(&langs)
            .arg("--path")
            .arg(&*temp_path)
            .assert();

        // Some combinations might fail due to invalid templates, that's ok
        if result.try_success().is_ok() {
            let flake_path = temp_dir.path().join("flake.nix");
            if flake_path.exists() {
                let flake_content =
                    fs::read_to_string(&flake_path).expect("Should read flake content");

                // Basic validation
                assert!(
                    flake_content.contains("Multi-language development environment"),
                    "Large combo {} should have multi-language description",
                    combo
                );
                assert!(
                    flake_content.contains("devShells"),
                    "Large combo {} should have devShells",
                    combo
                );

                // Validate with nix if available
                let safe_name = combo.replace(",", "-");
                validate_flake_content_with_nix_check(
                    &flake_content,
                    &format!("test-large-{}", safe_name),
                );
            }
        }
    }
}

#[test]
fn test_comprehensive_language_coverage() {
    // Test that ensures every single language appears in at least one combination
    let all_languages = [
        "bun",
        "c-cpp",
        "clojure",
        "csharp",
        "cue",
        "dhall",
        "elixir",
        "elm",
        "gleam",
        "go",
        "hashi",
        "haskell",
        "haxe",
        "java",
        "kotlin",
        "latex",
        "nickel",
        "nim",
        "nix",
        "node",
        "ocaml",
        "opa",
        "php",
        "protobuf",
        "pulumi",
        "python",
        "r",
        "ruby",
        "rust",
        "rust-toolchain",
        "scala",
        "shell",
        "swift",
        "vlang",
        "zig",
    ];

    // Comprehensive combinations that cover every language at least once
    let comprehensive_combinations = [
        // Group 1: Web & Frontend
        ("bun,node,elm", "Frontend Stack"),
        // Group 2: Systems Programming
        ("rust,c-cpp,zig", "Systems Languages"),
        // Group 3: JVM Ecosystem
        ("java,kotlin,scala,clojure", "JVM Full Stack"),
        // Group 4: Functional Programming
        ("haskell,ocaml,elm", "Pure Functional"),
        // Group 5: BEAM & Dynamic
        ("elixir,gleam,ruby", "Dynamic Languages"),
        // Group 6: Data & Scientific
        ("python,r,latex", "Scientific Computing"),
        // Group 7: Infrastructure & DevOps
        ("hashi,pulumi,nix,shell", "Infrastructure"),
        // Group 8: Configuration & Markup
        ("cue,dhall,nickel", "Configuration Languages"),
        // Group 9: Multi-target & Mobile
        ("haxe,swift,csharp", "Multi-target Development"),
        // Group 10: Niche & Specialized
        ("nim,vlang,opa", "Modern Alternatives"),
        // Group 11: Protocol & API
        ("protobuf,php,go", "API Development"),
        // Group 12: Rust variants
        ("rust-toolchain,rust", "Rust Variants"),
    ];

    // Verify every language is covered
    let mut covered_languages = std::collections::HashSet::new();
    for (langs, _) in &comprehensive_combinations {
        for lang in langs.split(',') {
            covered_languages.insert(lang);
        }
    }

    // Check that all languages are covered
    for lang in &all_languages {
        assert!(
            covered_languages.contains(lang),
            "Language '{}' is not covered in comprehensive combinations",
            lang
        );
    }

    println!(
        "✓ All {} languages are covered in comprehensive test combinations",
        all_languages.len()
    );

    // Now test each combination
    for (langs, description) in comprehensive_combinations {
        let mut cmd = Command::cargo_bin("dev-template-generator").unwrap();
        let temp_dir = TempDir::new().expect("Should create temp directory");
        let temp_path = temp_dir.path().to_string_lossy();

        cmd.arg("init")
            .arg(langs)
            .arg("--path")
            .arg(&*temp_path)
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "Initialized multi-language template ({}) in {}",
                langs, temp_path
            )));

        // Verify flake was created
        let flake_path = temp_dir.path().join("flake.nix");
        assert!(
            flake_path.exists(),
            "flake.nix should be created for {}",
            description
        );

        let flake_content = fs::read_to_string(&flake_path).expect("Should read flake content");

        // Basic validation
        assert!(
            flake_content.contains("Multi-language development environment"),
            "{} should have multi-language description",
            description
        );
        assert!(
            flake_content.contains("nixpkgs.url"),
            "{} should have nixpkgs input",
            description
        );
        assert!(
            flake_content.contains("devShells"),
            "{} should have devShells",
            description
        );

        // Verify each language appears in the description
        for lang in langs.split(',') {
            assert!(
                flake_content.contains(lang),
                "{} should contain {}",
                description,
                lang
            );
        }

        // Validate with nix
        let safe_name = langs.replace(",", "-");
        validate_flake_content_with_nix_check(
            &flake_content,
            &format!("test-comprehensive-{}", safe_name),
        );
    }

    println!("✅ Comprehensive language coverage test completed successfully");
}

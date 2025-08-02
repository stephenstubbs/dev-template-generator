use predicates::prelude::*;
use std::collections::HashSet;
use crate::integration::common::{
    create_cargo_command, create_temp_dir_with_path, assert_flake_exists_and_contains, 
    validate_flake_content_with_nix_check
};
use crate::integration::comprehensive_tests::ALL_LANGUAGES;

#[test]
fn test_comprehensive_language_coverage() {
    let comprehensive_combinations = [
        ("bun,node,elm", "Frontend Stack"),
        ("rust,c-cpp,zig", "Systems Languages"),
        ("java,kotlin,scala,clojure", "JVM Full Stack"),
        ("haskell,ocaml,elm", "Pure Functional"),
        ("elixir,gleam,ruby", "Dynamic Languages"),
        ("python,r,latex", "Scientific Computing"),
        ("hashi,pulumi,nix,shell", "Infrastructure"),
        ("cue,dhall,nickel", "Configuration Languages"),
        ("haxe,swift,csharp", "Multi-target Development"),
        ("nim,vlang,opa", "Modern Alternatives"),
        ("protobuf,php,go", "API Development"),
        ("rust-toolchain,rust", "Rust Variants"),
    ];

    verify_language_coverage(&comprehensive_combinations);

    for (langs, description) in comprehensive_combinations {
        test_comprehensive_combination(langs, description);
    }
    
    println!("✅ Comprehensive language coverage test completed successfully");
}

fn verify_language_coverage(combinations: &[(&str, &str)]) {
    let mut covered_languages = HashSet::new();
    for (langs, _) in combinations {
        for lang in langs.split(',') {
            covered_languages.insert(lang);
        }
    }
    
    for lang in ALL_LANGUAGES {
        assert!(
            covered_languages.contains(lang),
            "Language '{lang}' is not covered in comprehensive combinations"
        );
    }
    
    println!(
        "✓ All {} languages are covered in comprehensive test combinations",
        ALL_LANGUAGES.len()
    );
}

fn test_comprehensive_combination(langs: &str, description: &str) {
    let mut cmd = create_cargo_command();
    let (temp_dir, temp_path) = create_temp_dir_with_path();
    
    cmd.arg("init")
        .arg(langs)
        .arg("--path")
        .arg(&temp_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Initialized multi-language template ({langs}) in {temp_path}"
        )));
    
    let flake_content = assert_flake_exists_and_contains(&temp_dir, &[]);
    
    validate_comprehensive_structure(&flake_content, langs, description);
    
    let safe_name = langs.replace(",", "-");
    validate_flake_content_with_nix_check(
        &flake_content,
        &format!("test-comprehensive-{safe_name}"),
    );
}

fn validate_comprehensive_structure(flake_content: &str, langs: &str, description: &str) {
    assert!(
        flake_content.contains("Multi-language development environment"),
        "{description} should have multi-language description"
    );
    assert!(
        flake_content.contains("nixpkgs.url"),
        "{description} should have nixpkgs input"
    );
    assert!(
        flake_content.contains("devShells"),
        "{description} should have devShells"
    );
    
    for lang in langs.split(',') {
        assert!(
            flake_content.contains(lang),
            "{description} should contain {lang}"
        );
    }
}
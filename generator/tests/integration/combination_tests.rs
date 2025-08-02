use predicates::prelude::*;
use crate::integration::common::{
    create_cargo_command, create_temp_dir_with_path, assert_flake_exists_and_contains, 
    validate_flake_content_with_nix_check
};

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
        test_language_combination(langs, description);
    }
}

fn test_language_combination(langs: &str, description: &str) {
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
    
    validate_multi_language_structure(&flake_content, langs, description);
    
    let safe_name = langs.replace(",", "-");
    validate_flake_content_with_nix_check(&flake_content, &format!("test-combo-{safe_name}"));
}

fn validate_multi_language_structure(flake_content: &str, langs: &str, description: &str) {
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
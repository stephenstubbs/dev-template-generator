use once_cell::sync::Lazy;
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(RustEmbed)]
#[folder = "../nix-parser/src/templates/"]
struct Templates;

#[derive(Deserialize)]
struct TemplateMetadata {
    template: TemplateInfo,
}

#[derive(Deserialize)]
struct TemplateInfo {
    description: String,
}

pub static EMBEDDED_TEMPLATES: Lazy<HashMap<&'static str, (&'static str, &'static str)>> =
    Lazy::new(load_templates);

fn load_templates() -> HashMap<&'static str, (&'static str, &'static str)> {
    let mut templates = HashMap::new();

    // Get all embedded files
    for file_path in Templates::iter() {
        if file_path.ends_with(".toml") {
            // Extract template name from filename
            let template_name = file_path.strip_suffix(".toml").unwrap();

            // Read the TOML metadata
            if let Some(toml_file) = Templates::get(&file_path) {
                if let Ok(toml_content) = std::str::from_utf8(&toml_file.data) {
                    if let Ok(metadata) = toml::from_str::<TemplateMetadata>(toml_content) {
                        // Read the corresponding .nix file
                        let nix_path = format!("{template_name}.nix");
                        if let Some(nix_file) = Templates::get(&nix_path) {
                            if let Ok(nix_content) = std::str::from_utf8(&nix_file.data) {
                                // Convert to static strings by leaking memory
                                // This is acceptable for embedded templates that live for the program duration
                                let description: &'static str =
                                    Box::leak(metadata.template.description.into_boxed_str());
                                let content: &'static str =
                                    Box::leak(nix_content.to_string().into_boxed_str());
                                let name: &'static str =
                                    Box::leak(template_name.to_string().into_boxed_str());

                                templates.insert(name, (description, content));
                            }
                        }
                    }
                }
            }
        }
    }

    templates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_structure() {
        let templates = &*EMBEDDED_TEMPLATES;
        assert!(!templates.is_empty(), "Templates should not be empty");

        // Test that rust template exists
        assert!(templates.contains_key("rust"), "Rust template should exist");

        if let Some((description, content)) = templates.get("rust") {
            assert!(!description.is_empty(), "Description should not be empty");
            assert!(!content.is_empty(), "Content should not be empty");
            assert!(
                content.contains("rust-overlay"),
                "Rust template should contain rust-overlay"
            );
        }
    }

    #[test]
    fn test_all_templates_exist() {
        let templates = &*EMBEDDED_TEMPLATES;
        let expected_templates = [
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

        for template in &expected_templates {
            assert!(
                templates.contains_key(template),
                "Template '{template}' should exist"
            );
        }
    }

    #[test]
    fn test_rust_template_has_overlay() {
        let templates = &*EMBEDDED_TEMPLATES;
        if let Some((_, content)) = templates.get("rust") {
            assert!(
                content.contains("overlays.default"),
                "Rust template should have overlay"
            );
            assert!(
                content.contains("rustToolchain"),
                "Rust template should define rustToolchain"
            );
        }
    }

    #[test]
    fn test_go_template_version() {
        let templates = &*EMBEDDED_TEMPLATES;
        if let Some((_, content)) = templates.get("go") {
            assert!(
                content.contains("go"),
                "Go template should contain go package"
            );
        }
    }

    #[test]
    fn test_java_templates_have_jdk() {
        let templates = &*EMBEDDED_TEMPLATES;
        let java_templates = ["java", "kotlin", "scala"];

        for template_name in &java_templates {
            if let Some((_, content)) = templates.get(template_name) {
                // Java templates should reference JDK in some form
                assert!(
                    content.contains("jdk") || content.contains("openjdk"),
                    "{template_name} template should reference JDK"
                );
            }
        }
    }
}

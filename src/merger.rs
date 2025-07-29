use crate::template::Template;
use anyhow::{Result, anyhow};
use rnix::ast::HasEntry;
use rowan::ast::AstNode;
use std::collections::{HashMap, HashSet};

pub struct FlakeFragments {
    pub header: String,
    pub inputs: HashMap<String, String>,
    pub overlays: HashMap<String, String>,
    pub packages: HashSet<String>,
    pub env_vars: HashMap<String, String>,
    pub shell_hooks: HashSet<String>,
    pub allow_unfree: bool,
}

pub fn merge_templates(templates: &[Template]) -> Result<String> {
    if templates.is_empty() {
        return Err(anyhow!("No templates provided for merging"));
    }

    if templates.len() == 1 {
        return Ok(templates[0].flake_content.clone());
    }

    let mut fragments = FlakeFragments {
        header: String::new(),
        inputs: HashMap::new(),
        overlays: HashMap::new(),
        packages: HashSet::new(),
        env_vars: HashMap::new(),
        shell_hooks: HashSet::new(),
        allow_unfree: false,
    };

    let descriptions: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
    fragments.header = format!(
        "Multi-language development environment ({})",
        descriptions.join(", ")
    );

    for template in templates {
        parse_template(&template.flake_content, &mut fragments)?;
    }

    generate_merged_flake(&fragments)
}

fn parse_template(content: &str, fragments: &mut FlakeFragments) -> Result<()> {
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut in_overlay = false;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.starts_with("inputs") {
            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with("}") {
                let input_line = lines[i].trim();
                if input_line.contains(".url = ") || input_line.contains("rust-overlay = {") {
                    if let Some((key, url)) = extract_input_key(input_line) {
                        fragments.inputs.insert(key, url);
                    }
                }
                i += 1;
            }
        } else if line.contains("overlays.default = final: prev:")
            || line.contains("overlays.default = final: prev: rec {")
        {
            in_overlay = true;
            let template_name = extract_template_name_from_overlay(content);
            let overlay_content = extract_overlay_content(content, i)?;
            if !overlay_content.trim().is_empty() {
                fragments.overlays.insert(template_name, overlay_content);
            }
        } else if line.contains("packages =") && !in_overlay {
            // Handle single-line package definitions like "packages = with pkgs; [ zig ];"
            if line.contains("[") && line.contains("]") {
                // Special handling for complex elm template: (with pkgs.elmPackages; [ elm ]) ++ (with pkgs; [ elm2nix ])
                if line.contains("with pkgs.elmPackages") && line.contains("elm") {
                    fragments.packages.insert("elmPackages.elm".to_string());
                }
                if line.contains("elm2nix") {
                    fragments.packages.insert("elm2nix".to_string());
                }
                // Extract packages from the same line for simple cases (skip complex elm patterns)
                if !line.contains("with pkgs.elmPackages") {
                    if let Some(start) = line.find('[') {
                        if let Some(end) = line.find(']') {
                            let pkg_section = &line[start + 1..end];
                            for pkg in pkg_section.split_whitespace() {
                                let pkg = pkg.trim_matches(|c| c == ',' || c == ';');
                                if !pkg.is_empty() && !pkg.contains("with") && !pkg.contains("pkgs")
                                {
                                    fragments.packages.insert(pkg.to_string());
                                }
                            }
                        }
                    }
                }
                i += 1;
                continue;
            }

            i += 1;
            let mut brace_level = 0;
            let mut paren_level = 0;
            while i < lines.len() {
                let pkg_line = lines[i].trim();

                // Count braces and parentheses to handle nested structures
                brace_level += pkg_line.chars().filter(|&c| c == '[').count() as i32;
                brace_level -= pkg_line.chars().filter(|&c| c == ']').count() as i32;
                paren_level += pkg_line.chars().filter(|&c| c == '(').count() as i32;
                paren_level -= pkg_line.chars().filter(|&c| c == ')').count() as i32;

                if brace_level < 0 {
                    break; // End of packages array
                }

                if !pkg_line.starts_with('#')
                    && !pkg_line.is_empty()
                    && brace_level >= 0
                    && paren_level >= 0
                {
                    // Handle special package patterns
                    if pkg_line.contains("dhallTools") {
                        // Add common dhall packages
                        fragments.packages.insert("dhall".to_string());
                        fragments
                            .packages
                            .insert("haskellPackages.dhall-bash".to_string());
                        fragments
                            .packages
                            .insert("haskellPackages.dhall-docs".to_string());
                        fragments
                            .packages
                            .insert("haskellPackages.dhall-json".to_string());
                        fragments
                            .packages
                            .insert("haskellPackages.dhall-lsp-server".to_string());
                        fragments
                            .packages
                            .insert("haskellPackages.dhall-nix".to_string());
                        fragments
                            .packages
                            .insert("haskellPackages.dhall-nixpkgs".to_string());
                        fragments
                            .packages
                            .insert("haskellPackages.dhall-openapi".to_string());
                        fragments
                            .packages
                            .insert("haskellPackages.dhall-toml".to_string());
                        fragments
                            .packages
                            .insert("haskellPackages.dhall-yaml".to_string());
                    } else if pkg_line.contains("with pkgs.elmPackages") && pkg_line.contains("elm")
                    {
                        // Handle elm template special case: (with pkgs.elmPackages; [ elm ])
                        fragments.packages.insert("elmPackages.elm".to_string());
                    } else {
                        // Skip complex expressions but allow simple package names
                        if !pkg_line.contains("with")
                            && !pkg_line.contains("++")
                            && !pkg_line.contains("if")
                            && !pkg_line.contains("override")
                            && !pkg_line.contains("=")
                            && !pkg_line.contains("builtins")
                            && !pkg_line.contains("import")
                            && !pkg_line.contains("final")
                            && !pkg_line.contains("prev")
                        {
                            let pkg = pkg_line
                                .replace([',', ';', '(', ')'], "")
                                .trim()
                                .to_string();
                            if !pkg.is_empty()
                                && !pkg.starts_with('[')
                                && !pkg.starts_with(']')
                                && !pkg.starts_with('{')
                                && !pkg.starts_with('}')
                                && !pkg.contains('}')
                                && !pkg.starts_with('#')
                                && pkg.chars().all(|c| {
                                    c.is_alphanumeric() || c == '-' || c == '_' || c == '.'
                                })
                            {
                                fragments.packages.insert(pkg);
                            }
                        }
                    }
                }
                i += 1;
            }
        } else if line.contains("env = {") {
            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with("}") {
                let env_line = lines[i].trim();
                if let Some((key, value)) = parse_env_var(env_line) {
                    fragments.env_vars.insert(key, value);
                }
                i += 1;
            }
        } else if (line.contains("shellHook = ''") || line.contains("venvShellHook"))
            && (content.contains("python") || content.contains("python311"))
        {
            fragments.shell_hooks.insert("python-venv".to_string());
        }

        // Reset overlay state when exiting overlay block
        if in_overlay && line.contains("};") && !line.contains("overlays.default") {
            in_overlay = false;
        }

        i += 1;
    }

    // Check for allowUnfree configuration in the entire content
    if content.contains("config.allowUnfree = true") || content.contains("allowUnfree = true") {
        fragments.allow_unfree = true;
    }

    Ok(())
}

fn extract_template_name_from_overlay(content: &str) -> String {
    if content.contains("rustToolchain") {
        "rust".to_string()
    } else if content.contains("go_1_") {
        "go".to_string()
    } else if content.contains("nodejs") && content.contains("yarn") {
        "node".to_string()
    } else if content.contains("rEnv") || content.contains("rWrapper") {
        "r".to_string()
    } else if content.contains("jdk") && content.contains("maven") {
        "java".to_string()
    } else if content.contains("jdk") && content.contains("kotlin") {
        "kotlin".to_string()
    } else if content.contains("jdk") && content.contains("scala") {
        "scala".to_string()
    } else if content.contains("elixir") && content.contains("beam") {
        "elixir".to_string()
    } else {
        "unknown".to_string()
    }
}

fn extract_overlay_content(content: &str, _start_idx: usize) -> Result<String> {
    let mut result = HashMap::<String, String>::new();

    if content.contains("rustToolchain") {
        result.insert(
            "rustToolchain".to_string(),
            r#"        rustToolchain =
          let
            rust = prev.rust-bin;
          in
          if builtins.pathExists ./rust-toolchain.toml then
            rust.fromRustupToolchainFile ./rust-toolchain.toml
          else if builtins.pathExists ./rust-toolchain then
            rust.fromRustupToolchainFile ./rust-toolchain
          else
            rust.stable.latest.default.override {
              extensions = [
                "rust-src"
                "rustfmt"
              ];
            };
"#
            .to_string(),
        );
    }

    if content.contains("go_1_") {
        result.insert(
            "go".to_string(),
            "        go = final.go_1_24;\n".to_string(),
        );
    }

    if content.contains("nodejs") && content.contains("yarn") {
        result.insert(
            "nodejs".to_string(),
            "        nodejs = prev.nodejs;\n".to_string(),
        );
        result.insert(
            "yarn".to_string(),
            "        yarn = (prev.yarn.override { inherit (final) nodejs; });\n".to_string(),
        );
    }

    if content.contains("rEnv") || content.contains("rWrapper") {
        result.insert("rEnv".to_string(), "        rEnv = final.rWrapper.override { packages = with final.rPackages; [ knitr ]; };\n".to_string());
    }

    // JVM languages - deduplicate common attributes
    if content.contains("jdk") {
        result.insert(
            "jdk".to_string(),
            "        jdk = prev.\"jdk21\";\n".to_string(),
        );
    }

    if content.contains("maven") {
        result.insert(
            "maven".to_string(),
            "        maven = prev.maven;\n".to_string(),
        );
    }

    if content.contains("gradle") {
        result.insert(
            "gradle".to_string(),
            "        gradle = prev.gradle.override { java = final.jdk; };\n".to_string(),
        );
    }

    if content.contains("kotlin") && content.contains("jdk") {
        result.insert(
            "kotlin".to_string(),
            "        kotlin = prev.kotlin;\n".to_string(),
        );
    }

    if content.contains("scala") && content.contains("jdk") {
        result.insert("sbt".to_string(), "        sbt = prev.sbt;\n".to_string());
        result.insert(
            "scala".to_string(),
            "        scala = prev.scala_3;\n".to_string(),
        );
    }

    if content.contains("elixir") && content.contains("beam") {
        result.insert(
            "erlang".to_string(),
            "        erlang = final.beam.interpreters.erlang_27;\n".to_string(),
        );
        result.insert(
            "pkgs-beam".to_string(),
            "        pkgs-beam = final.beam.packagesWith final.erlang;\n".to_string(),
        );
        result.insert(
            "elixir".to_string(),
            "        elixir = final.pkgs-beam.elixir_1_17;\n".to_string(),
        );
    }

    // Sort and concatenate all overlay content
    let mut sorted_keys: Vec<_> = result.keys().collect();
    sorted_keys.sort();
    let final_result = sorted_keys
        .into_iter()
        .map(|key| result[key].clone())
        .collect::<String>();

    Ok(final_result)
}

/// Parse overlay content and extract attribute definitions using rnix with fallback for complex attributes
fn parse_overlay_attributes(overlay_content: &str) -> HashMap<String, String> {
    let mut attributes = HashMap::new();

    // First, handle complex multi-line attributes that need special string parsing
    let complex_attributes = parse_complex_attributes_with_strings(overlay_content);
    for (name, content) in complex_attributes {
        attributes.insert(name, content);
    }

    // Then use rnix for simple attributes
    let root = rnix::Root::parse(overlay_content);
    if root.errors().is_empty() {
        let syntax_node = root.syntax();
        traverse_for_attributes(&syntax_node, &mut attributes);
    }

    // If no attributes found, use complete fallback
    if attributes.is_empty() {
        return parse_overlay_attributes_fallback(overlay_content);
    }

    attributes
}

/// Parse complex multi-line attributes using string parsing
fn parse_complex_attributes_with_strings(overlay_content: &str) -> HashMap<String, String> {
    let mut attributes = HashMap::new();

    // Handle rustToolchain specifically - it's a complex multi-line let expression
    if overlay_content.contains("rustToolchain") {
        if let Some(rust_toolchain_def) = extract_rust_toolchain_definition(overlay_content) {
            attributes.insert("rustToolchain".to_string(), rust_toolchain_def);
        }
    }

    // Add other complex attributes here as needed

    attributes
}

/// Extract the complete rustToolchain definition using string parsing
fn extract_rust_toolchain_definition(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut result_lines = Vec::new();
    let mut in_rust_toolchain = false;
    let mut paren_count = 0;
    let mut brace_count = 0;
    let mut bracket_count = 0;

    for line in lines.iter() {
        let trimmed = line.trim();

        // Look for the start of rustToolchain definition
        if trimmed.contains("rustToolchain") && trimmed.contains("=") {
            in_rust_toolchain = true;
            result_lines.push(line.to_string());

            // If this line also contains the complete definition (single line), handle it
            if trimmed.ends_with(";") && !trimmed.contains("let") {
                break;
            }
            continue;
        }

        if in_rust_toolchain {
            result_lines.push(line.to_string());

            // Count all types of brackets/braces/parens
            paren_count += line.chars().filter(|&c| c == '(').count() as i32;
            paren_count -= line.chars().filter(|&c| c == ')').count() as i32;
            brace_count += line.chars().filter(|&c| c == '{').count() as i32;
            brace_count -= line.chars().filter(|&c| c == '}').count() as i32;
            bracket_count += line.chars().filter(|&c| c == '[').count() as i32;
            bracket_count -= line.chars().filter(|&c| c == ']').count() as i32;

            // Check if we've reached the end - all brackets closed and line ends with semicolon
            if trimmed.ends_with(";") && paren_count <= 0 && brace_count <= 0 && bracket_count <= 0
            {
                // Make sure this isn't just a semicolon inside the definition
                if trimmed.ends_with("};") || (trimmed.len() <= 20 && !trimmed.contains("=")) {
                    break;
                }
            }

            // Safety check - don't go beyond reasonable bounds
            if result_lines.len() > 50 {
                break;
            }
        }
    }

    if !result_lines.is_empty() {
        Some(result_lines.join("\n") + "\n")
    } else {
        None
    }
}

/// Recursively traverse syntax nodes to find attribute definitions
fn traverse_for_attributes(node: &rnix::SyntaxNode, attributes: &mut HashMap<String, String>) {
    // Check if this node is an attribute set
    if let Some(attr_set) = rnix::ast::AttrSet::cast(node.clone()) {
        // Extract attributes from this set
        for entry in attr_set.entries() {
            if let rnix::ast::Entry::AttrpathValue(attr_path_value) = entry {
                if let Some(attr_path) = attr_path_value.attrpath() {
                    // Get the first part of the attribute path as the name
                    if let Some(first_attr) = attr_path.attrs().next() {
                        let attr_name = first_attr.to_string();

                        // Skip attributes that were already handled by string parsing
                        if attributes.contains_key(&attr_name) {
                            continue;
                        }

                        // Get the value and format as an attribute definition
                        if let Some(value) = attr_path_value.value() {
                            let value_text = value.to_string();
                            let attr_content = format!("        {attr_name} = {value_text};\n");
                            attributes.insert(attr_name, attr_content);
                        }
                    }
                }
            }
        }
    }

    // Recursively traverse child nodes
    for child in node.children() {
        traverse_for_attributes(&child, attributes);
    }
}

/// Fallback parser for overlay attributes using line-by-line parsing
fn parse_overlay_attributes_fallback(overlay_content: &str) -> HashMap<String, String> {
    let mut attributes = HashMap::new();
    let lines: Vec<&str> = overlay_content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Look for attribute definitions that start with proper indentation
        if line.starts_with("        ") && trimmed.contains(" = ") {
            if let Some(eq_pos) = trimmed.find(" = ") {
                let attr_name = trimmed[..eq_pos].trim().to_string();

                // Handle single-line attributes
                if trimmed.ends_with(";") {
                    attributes.insert(attr_name, format!("{line}\n"));
                    i += 1;
                }
                // Handle multi-line attributes (like rustToolchain)
                else {
                    let mut attr_lines = vec![line];
                    i += 1;

                    // Collect lines until we find the end of the attribute
                    let mut brace_count = 0;
                    let mut found_end = false;

                    while i < lines.len() && !found_end {
                        let current_line = lines[i];
                        attr_lines.push(current_line);

                        // Count braces to handle nested structures
                        brace_count += current_line.chars().filter(|&c| c == '{').count() as i32;
                        brace_count -= current_line.chars().filter(|&c| c == '}').count() as i32;

                        // Check if this line ends the attribute
                        if (current_line.trim().ends_with("};")
                            || (current_line.trim().ends_with(";") && !current_line.contains("{")))
                            && brace_count <= 0
                        {
                            found_end = true;
                        }

                        i += 1;
                    }

                    let complete_attr = attr_lines.join("\n") + "\n";
                    attributes.insert(attr_name, complete_attr);
                }
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    attributes
}

fn extract_input_key(line: &str) -> Option<(String, String)> {
    if let Some(start) = line.find("url = \"") {
        if let Some(end) = line[start + 7..].find('"') {
            let url = &line[start + 7..start + 7 + end];

            let key = if line.contains("nixpkgs.url") {
                "nixpkgs".to_string()
            } else if line.contains("rust-overlay") {
                "rust-overlay".to_string()
            } else {
                url.split('/')
                    .next_back()
                    .unwrap_or("unknown")
                    .replace('#', "_")
                    .to_string()
            };

            return Some((key, url.to_string()));
        }
    } else if line.contains("rust-overlay = {") {
        return Some((
            "rust-overlay".to_string(),
            "github:oxalica/rust-overlay".to_string(),
        ));
    }
    None
}

fn parse_env_var(line: &str) -> Option<(String, String)> {
    if let Some(eq_pos) = line.find(" = ") {
        let key = line[..eq_pos].trim().to_string();
        let value = line[eq_pos + 3..].trim().trim_end_matches(';').to_string();
        Some((key, value))
    } else {
        None
    }
}

fn generate_merged_flake(fragments: &FlakeFragments) -> Result<String> {
    let mut inputs_fragment = String::new();

    // Always include nixpkgs
    inputs_fragment.push_str(
        r#"    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
"#,
    );

    for (key, url) in &fragments.inputs {
        if key == "nixpkgs" {
            continue; // Already added above
        } else if key == "rust-overlay" {
            inputs_fragment.push_str(&format!(
                r#"    {key} = {{
      url = "{url}";
      inputs.nixpkgs.follows = "nixpkgs";
    }};
"#
            ));
        } else {
            inputs_fragment.push_str(&format!(
                r#"    {key}.url = "{url}";
"#
            ));
        }
    }

    let mut overlays_fragment = String::new();
    if !fragments.overlays.is_empty() {
        overlays_fragment.push_str("      overlays.default = final: prev: {\n");

        // Parse and deduplicate overlay attributes using a more robust approach
        let mut final_attributes = HashMap::<String, String>::new();

        for overlay_content in fragments.overlays.values() {
            let attributes = parse_overlay_attributes(overlay_content);
            for (attr_name, attr_content) in attributes {
                final_attributes.insert(attr_name, attr_content);
            }
        }

        // Sort and add deduplicated attributes
        let mut sorted_attrs: Vec<_> = final_attributes.keys().collect();
        sorted_attrs.sort();
        for attr_name in sorted_attrs {
            overlays_fragment.push_str(&final_attributes[attr_name]);
        }

        overlays_fragment.push_str("      };\n");
    }

    let mut packages_fragment = String::new();
    let mut sorted_packages: Vec<_> = fragments.packages.iter().collect();
    sorted_packages.sort();
    for package in sorted_packages {
        packages_fragment.push_str(&format!("              {package}\n"));
    }

    let env_fragment = if !fragments.env_vars.is_empty() {
        let mut env_content = String::from("\n            env = {\n");
        for (key, value) in &fragments.env_vars {
            env_content.push_str(&format!("              {key} = {value};\n"));
        }
        env_content.push_str("            };");
        env_content
    } else {
        String::new()
    };

    let shell_hook_fragment = if !fragments.shell_hooks.is_empty() {
        let mut hook_content = String::new();
        for hook in &fragments.shell_hooks {
            if hook.as_str() == "python-venv" {
                hook_content.push_str(
                    r#"
            shellHook = ''
              # Create virtual environment if it doesn't exist
              if [ ! -d ".venv" ]; then
                python -m venv .venv
              fi
              
              # Activate virtual environment
              source .venv/bin/activate
              
              # Upgrade pip in virtual environment
              pip install --upgrade pip
            '';"#,
                );
            }
        }
        hook_content
    } else {
        String::new()
    };

    let input_names = fragments
        .inputs
        .keys()
        .filter(|k| *k != "nixpkgs")
        .map(|k| format!("\n      {k},"))
        .collect::<String>();

    let overlay_refs = if fragments.inputs.contains_key("rust-overlay") {
        "\n                rust-overlay.overlays.default"
    } else {
        ""
    };

    let pkgs_import_section = if !fragments.overlays.is_empty() {
        if fragments.allow_unfree {
            format!(
                "            pkgs = import nixpkgs {{
              inherit system;
              config.allowUnfree = true;
              overlays = [{overlay_refs}
                self.overlays.default
              ];
            }};"
            )
        } else {
            format!(
                "            pkgs = import nixpkgs {{
              inherit system;
              overlays = [{overlay_refs}
                self.overlays.default
              ];
            }};"
            )
        }
    } else if fragments.allow_unfree {
        "            pkgs = import nixpkgs { inherit system; config.allowUnfree = true; };"
            .to_string()
    } else {
        "            pkgs = import nixpkgs { inherit system; };".to_string()
    };

    let flake = format!(
        r#"{{
  description = "{}";

  inputs = {{
{}  }};

  outputs =
    {{
      self,
      nixpkgs,{}
    }}:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {{
{}
          }}
        );
    in
    {{
{}
      devShells = forEachSupportedSystem (
        {{ pkgs }}:
        {{
          default = pkgs.mkShell {{
            packages = with pkgs; [
{}            ];{}{}
          }};
        }}
      );
    }};
}}
"#,
        fragments.header,
        inputs_fragment,
        input_names,
        pkgs_import_section,
        overlays_fragment,
        packages_fragment,
        env_fragment,
        shell_hook_fragment
    );

    Ok(flake)
}

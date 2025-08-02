use crate::template::Template;
use anyhow::{Result, anyhow};
use nix_parser::{extract_flake_fragments, Binding, AttrPath, AttrPathPart};
use std::collections::{HashMap, HashSet};

pub struct FlakeFragments {
    pub header: String,
    pub inputs: HashMap<String, String>,
    pub overlays: HashMap<String, Vec<Binding>>, // Now using AST bindings
    pub packages: HashSet<String>,
    pub env_vars: HashMap<String, String>,
    pub shell_hooks: HashSet<String>,
    pub allow_unfree: bool,
    pub let_bindings: HashMap<String, String>,
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
        let_bindings: HashMap::new(),
    };

    let descriptions: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
    fragments.header = format!(
        "Multi-language development environment ({})",
        descriptions.join(", ")
    );

    for template in templates {
        parse_template_with_nix_parser(&template.flake_content, &mut fragments)?;
    }

    generate_merged_flake(&fragments)
}

fn parse_template_with_nix_parser(content: &str, fragments: &mut FlakeFragments) -> Result<()> {
    let parsed_fragments = extract_flake_fragments(content)
        .map_err(|e| anyhow!("Failed to parse nix template: {}", e))?;

    // Merge inputs
    for (key, value) in parsed_fragments.inputs {
        fragments.inputs.insert(key, value);
    }

    // Merge overlays - deduplicate bindings at AST level
    for (key, bindings) in parsed_fragments.overlays {
        if let Some(existing_bindings) = fragments.overlays.get_mut(&key) {
            merge_overlay_bindings(existing_bindings, bindings);
        } else {
            fragments.overlays.insert(key, bindings);
        }
    }

    // Merge packages (convert Vec to HashSet)
    // Note: We don't filter out overlay-defined packages anymore since they're actually 
    // available for use once the overlay is applied
    for package in parsed_fragments.packages {
        fragments.packages.insert(package);
    }

    // Merge environment variables
    for (key, value) in parsed_fragments.env_vars {
        fragments.env_vars.insert(key, value);
    }

    // Merge shell hooks (convert Vec to HashSet)
    for hook in parsed_fragments.shell_hooks {
        fragments.shell_hooks.insert(hook);
    }

    // Set allow_unfree if any template requires it
    if parsed_fragments.allow_unfree {
        fragments.allow_unfree = true;
    }

    // Merge let bindings
    for (key, value) in parsed_fragments.let_bindings {
        fragments.let_bindings.insert(key, value);
    }

    Ok(())
}



fn generate_merged_flake(fragments: &FlakeFragments) -> Result<String> {
    let mut inputs_fragment = String::new();

    // Generate inputs from extracted data
    let mut sorted_inputs: Vec<_> = fragments.inputs.iter().collect();
    sorted_inputs.sort_by_key(|(name, _)| *name);
    
    for (key, url) in sorted_inputs {
        if key.contains("overlay") {
            // Handle overlay inputs with follows pattern
            inputs_fragment.push_str(&format!(
                r#"    {key} = {{
      url = "{url}";
      inputs.nixpkgs.follows = "nixpkgs";
    }};
"#
            ));
        } else {
            // Simple URL inputs
            inputs_fragment.push_str(&format!(
                r#"    {key}.url = "{url}";
"#
            ));
        }
    }

    let mut overlays_fragment = String::new();
    if !fragments.overlays.is_empty() {
        overlays_fragment.push_str("      overlays.default = final: prev: rec {\n");

        // Generate overlay content from AST bindings
        let mut sorted_overlays: Vec<_> = fragments.overlays.iter().collect();
        sorted_overlays.sort_by_key(|(name, _)| *name);
        
        for (_, bindings) in sorted_overlays {
            for binding in bindings {
                overlays_fragment.push_str(&format!("        {} = {};\n", 
                    format_attr_path(&binding.path), 
                    binding.value.to_nix_string()));
            }
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

    // Generate let bindings fragment
    let let_bindings_fragment = if !fragments.let_bindings.is_empty() {
        let mut bindings_content = String::new();
        let mut sorted_bindings: Vec<_> = fragments.let_bindings.iter().collect();
        sorted_bindings.sort_by_key(|(name, _)| *name);
        
        for (name, value) in sorted_bindings {
            bindings_content.push_str(&format!("      {name} = {value};\n"));
        }
        bindings_content
    } else {
        String::new()
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
{}      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {{
            pkgs = import nixpkgs {{
              inherit system;{}
            }};
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
        let_bindings_fragment,
        generate_pkgs_config(fragments),
        overlays_fragment,
        packages_fragment,
        env_fragment,
        shell_hook_fragment
    );

    Ok(flake)
}


fn merge_overlay_bindings(existing: &mut Vec<Binding>, new_bindings: Vec<Binding>) {
    let mut existing_paths = HashSet::new();
    
    // Track existing binding paths
    for binding in existing.iter() {
        existing_paths.insert(format_attr_path(&binding.path));
    }
    
    // Add new bindings that don't conflict
    for binding in new_bindings {
        let path_str = format_attr_path(&binding.path);
        if !existing_paths.contains(&path_str) {
            existing_paths.insert(path_str);
            existing.push(binding);
        }
        // If there's a conflict, we keep the existing binding (first one wins)
    }
}

fn format_attr_path(path: &AttrPath) -> String {
    path.parts.iter()
        .map(|part| match part {
            AttrPathPart::Identifier(id) => id.clone(),
            AttrPathPart::String(s) => format!("\"{s}\""),
            AttrPathPart::Interpolation(expr) => format!("${{{}}}", expr.to_nix_string()),
        })
        .collect::<Vec<_>>()
        .join(".")
}


fn generate_pkgs_config(fragments: &FlakeFragments) -> String {
    // Generate overlay references from inputs dynamically
    let overlay_refs: Vec<String> = fragments.inputs.keys()
        .filter(|key| key.contains("overlay"))
        .map(|key| format!("\n                {key}.overlays.default"))
        .collect();
    
    let overlay_refs_str = overlay_refs.join("");

    if !fragments.overlays.is_empty() {
        if fragments.allow_unfree {
            format!(
                "\n              config.allowUnfree = true;
              overlays = [{overlay_refs_str}
                self.overlays.default
              ];")
        } else {
            format!(
                "\n              overlays = [{overlay_refs_str}
                self.overlays.default
              ];")
        }
    } else if fragments.allow_unfree {
        "\n              config.allowUnfree = true;".to_string()
    } else {
        String::new()
    }
}

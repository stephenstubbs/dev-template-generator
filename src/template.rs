use crate::embedded_templates::EMBEDDED_TEMPLATES;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub description: String,
    pub flake_content: String,
    pub additional_files: HashMap<String, String>,
}

pub struct TemplateManager {
    templates: HashMap<String, Template>,
}

impl TemplateManager {
    pub async fn new() -> Result<Self> {
        let mut manager = Self {
            templates: HashMap::new(),
        };

        manager.load_embedded_templates().await?;
        Ok(manager)
    }

    async fn load_embedded_templates(&mut self) -> Result<()> {
        for (template_name, (description, flake_content)) in EMBEDDED_TEMPLATES.iter() {
            let mut additional_files = HashMap::new();

            // Special case for rust-toolchain template - add sample rust-toolchain.toml
            if *template_name == "rust-toolchain" {
                additional_files.insert(
                    "rust-toolchain.toml".to_string(),
                    r#"[toolchain]
channel = "stable"
components = ["rustfmt", "rust-analyzer"]
"#
                    .to_string(),
                );
            }

            let template = Template {
                name: template_name.to_string(),
                description: description.to_string(),
                flake_content: flake_content.to_string(),
                additional_files,
            };

            self.templates.insert(template_name.to_string(), template);
        }

        Ok(())
    }

    pub async fn init_single(&self, template_name: &str, target_path: &Path) -> Result<()> {
        let template = self
            .templates
            .get(template_name)
            .ok_or_else(|| anyhow!("Template '{}' not found", template_name))?;

        fs::create_dir_all(target_path)?;

        fs::write(target_path.join("flake.nix"), &template.flake_content)?;

        for (filename, content) in &template.additional_files {
            fs::write(target_path.join(filename), content)?;
        }

        Ok(())
    }

    pub async fn init_multi(&self, template_names: &[&str], target_path: &Path) -> Result<()> {
        let mut templates = Vec::new();
        for name in template_names {
            let template = self
                .templates
                .get(*name)
                .ok_or_else(|| anyhow!("Template '{}' not found", name))?;
            templates.push(template.clone());
        }

        let merged = crate::merger::merge_templates(&templates)?;

        fs::create_dir_all(target_path)?;
        fs::write(target_path.join("flake.nix"), merged)?;

        for template in &templates {
            for (filename, content) in &template.additional_files {
                let target_file = target_path.join(filename);
                if !target_file.exists() {
                    fs::write(target_file, content)?;
                }
            }
        }

        Ok(())
    }

    pub fn list_templates(&self) {
        println!("Available templates:");
        let mut sorted: Vec<_> = self.templates.values().collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));

        for template in sorted {
            println!("  {} - {}", template.name, template.description);
        }
    }

    pub async fn update_templates(&mut self) -> Result<()> {
        self.templates.clear();
        self.load_embedded_templates().await?;
        Ok(())
    }
}

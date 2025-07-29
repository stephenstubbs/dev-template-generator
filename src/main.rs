use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod embedded_templates;
mod merger;
mod template;

use template::TemplateManager;

#[derive(Parser)]
#[command(name = "dev-template-generator")]
#[command(about = "Generate development environments from nix templates")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a template (single or multi-language)
    Init {
        /// Template name(s) - single template (e.g., 'rust') or comma-separated list for multi-language (e.g., 'rust,go,node')
        templates: String,
        /// Target directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// List available templates
    List,
    /// Update local template cache
    Update,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut manager = TemplateManager::new().await?;

    match cli.command {
        Commands::Init { templates, path } => {
            let target_path = path.unwrap_or_else(|| PathBuf::from("."));

            // Check if it's a single template or multiple templates
            if templates.contains(',') {
                // Multi-language template
                let template_list: Vec<&str> = templates.split(',').map(|s| s.trim()).collect();
                manager.init_multi(&template_list, &target_path).await?;
                println!(
                    "Initialized multi-language template ({}) in {}",
                    templates,
                    target_path.display()
                );
            } else {
                // Single template
                manager.init_single(&templates, &target_path).await?;
                println!(
                    "Initialized {} template in {}",
                    templates,
                    target_path.display()
                );
            }
        }
        Commands::List => {
            manager.list_templates();
        }
        Commands::Update => {
            manager.update_templates().await?;
            println!("Templates updated successfully");
        }
    }

    Ok(())
}

use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::fs;
use std::path::Path;

use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_core_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::{TemplateDeleteOutput, TemplatesConfig};

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Template name")]
    pub name: Option<String>,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,

    #[arg(long, help = "Also delete the .html file")]
    pub delete_file: bool,
}

pub fn execute(args: DeleteArgs, config: &CliConfig) -> Result<CommandResult<TemplateDeleteOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let web_path = profile.paths.web_path_resolved();
    let templates_dir = Path::new(&web_path).join("templates");
    let templates_yaml_path = templates_dir.join("templates.yaml");

    let yaml_content = fs::read_to_string(&templates_yaml_path).with_context(|| {
        format!(
            "Failed to read templates config at {}",
            templates_yaml_path.display()
        )
    })?;

    let mut templates_config: TemplatesConfig =
        serde_yaml::from_str(&yaml_content).with_context(|| {
            format!(
                "Failed to parse templates config at {}",
                templates_yaml_path.display()
            )
        })?;

    let name = resolve_input(args.name, "name", config, || {
        prompt_template_selection(&templates_config)
    })?;

    if !templates_config.templates.contains_key(&name) {
        return Err(anyhow!("Template '{}' not found", name));
    }

    if !args.yes {
        if !config.is_interactive() {
            return Err(anyhow!(
                "--yes is required to delete templates in non-interactive mode"
            ));
        }

        let confirm_msg = if args.delete_file {
            format!("Delete template '{}' and its HTML file?", name)
        } else {
            format!("Delete template '{}'?", name)
        };

        if !CliService::confirm(&confirm_msg)? {
            CliService::info("Cancelled");
            return Ok(CommandResult::text(TemplateDeleteOutput {
                deleted: String::new(),
                file_deleted: false,
                message: "Operation cancelled".to_string(),
            })
            .with_title("Delete Cancelled"));
        }
    }

    templates_config.templates.remove(&name);

    // Handle HTML file deletion
    let html_file_path = templates_dir.join(format!("{}.html", name));
    let file_deleted = if args.delete_file && html_file_path.exists() {
        fs::remove_file(&html_file_path).with_context(|| {
            format!("Failed to delete HTML file: {}", html_file_path.display())
        })?;
        true
    } else {
        false
    };

    // Write updated config
    let yaml = serde_yaml::to_string(&templates_config).context("Failed to serialize config")?;
    fs::write(&templates_yaml_path, yaml).with_context(|| {
        format!(
            "Failed to write templates config to {}",
            templates_yaml_path.display()
        )
    })?;

    let message = if file_deleted {
        format!(
            "Template '{}' deleted (including HTML file)",
            name
        )
    } else if html_file_path.exists() {
        format!(
            "Template '{}' deleted. HTML file still exists at {}",
            name,
            html_file_path.display()
        )
    } else {
        format!("Template '{}' deleted", name)
    };

    CliService::success(&message);

    let output = TemplateDeleteOutput {
        deleted: name,
        file_deleted,
        message,
    };

    Ok(CommandResult::text(output).with_title("Template Deleted"))
}

fn prompt_template_selection(config: &TemplatesConfig) -> Result<String> {
    let mut names: Vec<&String> = config.templates.keys().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No templates configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select template to delete")
        .items(&names)
        .default(0)
        .interact()
        .context("Failed to get template selection")?;

    Ok(names[selection].clone())
}

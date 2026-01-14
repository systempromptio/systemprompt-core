use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::fs;

use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_core_logging::CliService;
use systemprompt_models::content_config::ContentConfigRaw;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::ContentTypeDeleteOutput;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Content type name")]
    pub name: Option<String>,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub fn execute(args: DeleteArgs, config: &CliConfig) -> Result<CommandResult<ContentTypeDeleteOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let content_config_path = profile.paths.content_config();

    let content = fs::read_to_string(&content_config_path)
        .with_context(|| format!("Failed to read content config at {}", content_config_path))?;

    let mut content_config: ContentConfigRaw = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse content config at {}", content_config_path))?;

    let name = resolve_input(args.name, "name", config, || {
        prompt_content_type_selection(&content_config)
    })?;

    if !content_config.content_sources.contains_key(&name) {
        return Err(anyhow!("Content type '{}' not found", name));
    }

    if !args.yes {
        if !config.is_interactive() {
            return Err(anyhow!(
                "--yes is required to delete content types in non-interactive mode"
            ));
        }

        if !CliService::confirm(&format!("Delete content type '{}'?", name))? {
            CliService::info("Cancelled");
            return Ok(CommandResult::text(ContentTypeDeleteOutput {
                deleted: vec![],
                message: "Operation cancelled".to_string(),
            })
            .with_title("Delete Cancelled"));
        }
    }

    let web_config_path = profile.paths.web_config();
    if let Ok(web_content) = fs::read_to_string(&web_config_path) {
        if web_content.contains(&format!("- {}", name)) {
            CliService::warning(&format!(
                "Content type '{}' is referenced in web config. You may need to update {}",
                name, web_config_path
            ));
        }
    }

    content_config.content_sources.remove(&name);

    CliService::info(&format!("Deleting content type '{}'...", name));

    let yaml = serde_yaml::to_string(&content_config).context("Failed to serialize config")?;
    fs::write(&content_config_path, yaml)
        .with_context(|| format!("Failed to write content config to {}", content_config_path))?;

    CliService::success(&format!("Content type '{}' deleted successfully", name));

    let output = ContentTypeDeleteOutput {
        deleted: vec![name.clone()],
        message: format!("Content type '{}' deleted successfully", name),
    };

    Ok(CommandResult::text(output).with_title("Content Type Deleted"))
}

fn prompt_content_type_selection(config: &ContentConfigRaw) -> Result<String> {
    let mut names: Vec<&String> = config.content_sources.keys().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No content types configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select content type to delete")
        .items(&names)
        .default(0)
        .interact()
        .context("Failed to get content type selection")?;

    Ok(names[selection].clone())
}

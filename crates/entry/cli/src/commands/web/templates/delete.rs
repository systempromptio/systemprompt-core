//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::fs;
use std::path::Path;

use crate::CliConfig;
use crate::interactive::{Prompter, require_confirmation, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_logging::CliService;

use super::super::paths::WebPaths;
use super::super::types::{TemplateDeleteOutput, TemplatesConfig};
use super::selection::prompt_template_selection;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Template name")]
    pub name: Option<String>,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,

    #[arg(long, help = "Also delete the .html file")]
    pub delete_file: bool,
}

pub(super) fn execute(
    args: DeleteArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    execute_in_dir(args, prompter, config, &WebPaths::resolve()?.templates)
}

pub fn execute_in_dir(
    args: DeleteArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
    templates_dir: &Path,
) -> Result<CommandOutput> {
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

    let name = resolve_required(args.name, "name", config, || {
        prompt_template_selection(prompter, &templates_config, "Select template to delete")
    })?;

    if !templates_config.templates.contains_key(&name) {
        return Err(anyhow!("Template '{}' not found", name));
    }

    let confirm_msg = if args.delete_file {
        format!("Delete template '{}' and its HTML file?", name)
    } else {
        format!("Delete template '{}'?", name)
    };

    require_confirmation(prompter, &confirm_msg, args.yes, config)?;

    templates_config.templates.remove(&name);

    let html_file_path = templates_dir.join(format!("{}.html", name));
    let file_deleted = if args.delete_file && html_file_path.exists() {
        fs::remove_file(&html_file_path)
            .with_context(|| format!("Failed to delete HTML file: {}", html_file_path.display()))?;
        true
    } else {
        false
    };

    let yaml = serde_yaml::to_string(&templates_config).context("Failed to serialize config")?;
    fs::write(&templates_yaml_path, yaml).with_context(|| {
        format!(
            "Failed to write templates config to {}",
            templates_yaml_path.display()
        )
    })?;

    let message = if file_deleted {
        format!("Template '{}' deleted (including HTML file)", name)
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

    Ok(CommandOutput::card_value("Template Deleted", &output))
}

use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use crate::CliConfig;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use systemprompt_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::{TemplateCreateOutput, TemplateEntry, TemplatesConfig};

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Template name")]
    pub name: Option<String>,

    #[arg(long, help = "Content types to link (comma-separated)")]
    pub content_types: Option<String>,

    #[arg(long, help = "HTML content (use '-' to read from stdin)")]
    pub content: Option<String>,
}

pub fn execute(
    args: CreateArgs,
    config: &CliConfig,
) -> Result<CommandResult<TemplateCreateOutput>> {
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

    let name = resolve_required(args.name, "name", config, prompt_name)?;

    if templates_config.templates.contains_key(&name) {
        return Err(anyhow!("Template '{}' already exists", name));
    }

    let content_types: Vec<String> = if let Some(ct) = args.content_types {
        ct.split(',').map(|s| s.trim().to_string()).collect()
    } else if config.is_interactive() {
        prompt_content_types()?
    } else {
        return Err(anyhow!(
            "--content-types is required in non-interactive mode"
        ));
    };

    if content_types.is_empty() {
        return Err(anyhow!("At least one content type is required"));
    }

    let html_file_path = templates_dir.join(format!("{}.html", name));

    let html_written = if let Some(content_source) = &args.content {
        let html_content = if content_source == "-" {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .context("Failed to read from stdin")?;
            buffer
        } else if Path::new(content_source).exists() {
            fs::read_to_string(content_source)
                .with_context(|| format!("Failed to read file: {}", content_source))?
        } else {
            content_source.clone()
        };

        fs::write(&html_file_path, html_content)
            .with_context(|| format!("Failed to write HTML file: {}", html_file_path.display()))?;
        true
    } else {
        false
    };

    templates_config
        .templates
        .insert(name.clone(), TemplateEntry { content_types });

    let yaml = serde_yaml::to_string(&templates_config).context("Failed to serialize config")?;
    fs::write(&templates_yaml_path, yaml).with_context(|| {
        format!(
            "Failed to write templates config to {}",
            templates_yaml_path.display()
        )
    })?;

    let message = if html_written {
        format!(
            "Template '{}' created with HTML file at {}",
            name,
            html_file_path.display()
        )
    } else {
        format!(
            "Template '{}' created. Create HTML file at {}",
            name,
            html_file_path.display()
        )
    };

    CliService::success(&message);

    let output = TemplateCreateOutput {
        name,
        file_path: html_file_path.to_string_lossy().to_string(),
        message,
    };

    Ok(CommandResult::text(output).with_title("Template Created"))
}

fn prompt_name() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Template name")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.len() < 2 {
                return Err("Name must be at least 2 characters");
            }
            if !input
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err("Name must be lowercase alphanumeric with hyphens only");
            }
            Ok(())
        })
        .interact_text()
        .context("Failed to get name")
}

fn prompt_content_types() -> Result<Vec<String>> {
    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Content types (comma-separated)")
        .interact_text()
        .context("Failed to get content types")?;

    Ok(input.split(',').map(|s| s.trim().to_string()).collect())
}

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use crate::CliConfig;
use crate::interactive::{Prompter, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_logging::CliService;

use super::super::paths::WebPaths;
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

pub(super) fn execute(
    args: CreateArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let web_paths = WebPaths::resolve()?;
    let templates_dir = &web_paths.templates;
    let templates_yaml_path = templates_dir.join("templates.yaml");

    let mut templates_config = load_templates_config(&templates_yaml_path)?;

    let name = resolve_required(args.name, "name", config, || prompt_name(prompter))?;

    if templates_config.templates.contains_key(&name) {
        return Err(anyhow!("Template '{}' already exists", name));
    }

    let content_types = resolve_content_types(args.content_types, prompter, config)?;

    let html_file_path = templates_dir.join(format!("{}.html", name));

    let html_written = if let Some(content_source) = &args.content {
        let html_content = read_html_content(content_source)?;
        fs::write(&html_file_path, html_content)
            .with_context(|| format!("Failed to write HTML file: {}", html_file_path.display()))?;
        true
    } else {
        false
    };

    templates_config
        .templates
        .insert(name.clone(), TemplateEntry { content_types });

    save_templates_config(&templates_yaml_path, &templates_config)?;

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

    Ok(CommandOutput::card_value("Template Created", &output))
}

fn load_templates_config(templates_yaml_path: &Path) -> Result<TemplatesConfig> {
    let yaml_content = fs::read_to_string(templates_yaml_path).with_context(|| {
        format!(
            "Failed to read templates config at {}",
            templates_yaml_path.display()
        )
    })?;

    serde_yaml::from_str(&yaml_content).with_context(|| {
        format!(
            "Failed to parse templates config at {}",
            templates_yaml_path.display()
        )
    })
}

fn save_templates_config(templates_yaml_path: &Path, config: &TemplatesConfig) -> Result<()> {
    let yaml = serde_yaml::to_string(config).context("Failed to serialize config")?;
    fs::write(templates_yaml_path, yaml).with_context(|| {
        format!(
            "Failed to write templates config to {}",
            templates_yaml_path.display()
        )
    })
}

fn resolve_content_types(
    arg: Option<String>,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<Vec<String>> {
    let content_types: Vec<String> = if let Some(ct) = arg {
        ct.split(',').map(|s| s.trim().to_owned()).collect()
    } else if config.is_interactive() {
        prompt_content_types(prompter)?
    } else {
        return Err(anyhow!(
            "--content-types is required in non-interactive mode"
        ));
    };

    if content_types.is_empty() {
        return Err(anyhow!("At least one content type is required"));
    }

    Ok(content_types)
}

fn read_html_content(content_source: &str) -> Result<String> {
    if content_source == "-" {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        Ok(buffer)
    } else if Path::new(content_source).exists() {
        fs::read_to_string(content_source)
            .with_context(|| format!("Failed to read file: {}", content_source))
    } else {
        Ok(content_source.to_owned())
    }
}

pub fn prompt_name(prompter: &dyn Prompter) -> Result<String> {
    loop {
        let input = prompter.input("Template name")?;
        let trimmed = input.trim();
        match validate_template_name(trimmed) {
            Ok(()) => return Ok(trimmed.to_owned()),
            Err(message) => CliService::warning(message),
        }
    }
}

fn validate_template_name(input: &str) -> Result<(), &'static str> {
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
}

pub fn prompt_content_types(prompter: &dyn Prompter) -> Result<Vec<String>> {
    let input = prompter.input("Content types (comma-separated)")?;
    Ok(input.split(',').map(|s| s.trim().to_owned()).collect())
}

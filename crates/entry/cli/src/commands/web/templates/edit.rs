use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_core_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::{TemplateEditOutput, TemplatesConfig};

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Template name")]
    pub name: Option<String>,

    #[arg(long, help = "Add content type to template")]
    pub add_content_type: Option<String>,

    #[arg(long, help = "Remove content type from template")]
    pub remove_content_type: Option<String>,

    #[arg(long, help = "Replace HTML content (use '-' for stdin)")]
    pub content: Option<String>,

    #[arg(long, help = "Set content types (comma-separated, replaces existing)")]
    pub content_types: Option<String>,
}

pub fn execute(args: EditArgs, config: &CliConfig) -> Result<CommandResult<TemplateEditOutput>> {
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

    let entry = templates_config
        .templates
        .get_mut(&name)
        .ok_or_else(|| anyhow!("Template '{}' not found", name))?;

    let mut changes = Vec::new();

    if let Some(ct) = args.content_types {
        let new_types: Vec<String> = ct.split(',').map(|s| s.trim().to_string()).collect();
        entry.content_types.clone_from(&new_types);
        changes.push(format!("content_types: {:?}", new_types));
    }

    if let Some(add_type) = &args.add_content_type {
        if entry.content_types.contains(add_type) {
            CliService::warning(&format!(
                "Content type '{}' already linked to template",
                add_type
            ));
        } else {
            entry.content_types.push(add_type.clone());
            changes.push(format!("added content_type: {}", add_type));
        }
    }

    if let Some(remove_type) = &args.remove_content_type {
        if let Some(pos) = entry.content_types.iter().position(|x| x == remove_type) {
            entry.content_types.remove(pos);
            changes.push(format!("removed content_type: {}", remove_type));
        } else {
            return Err(anyhow!(
                "Content type '{}' not linked to template",
                remove_type
            ));
        }
    }

    if let Some(content_source) = &args.content {
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

        let html_file_path = templates_dir.join(format!("{}.html", name));
        fs::write(&html_file_path, html_content)
            .with_context(|| format!("Failed to write HTML file: {}", html_file_path.display()))?;
        changes.push(format!("updated HTML file: {}", html_file_path.display()));
    }

    if changes.is_empty() {
        return Err(anyhow!(
            "No changes specified. Use --add-content-type, --remove-content-type, \
             --content-types, or --content"
        ));
    }

    let yaml = serde_yaml::to_string(&templates_config).context("Failed to serialize config")?;
    fs::write(&templates_yaml_path, yaml).with_context(|| {
        format!(
            "Failed to write templates config to {}",
            templates_yaml_path.display()
        )
    })?;

    CliService::success(&format!("Template '{}' updated successfully", name));

    let output = TemplateEditOutput {
        name: name.clone(),
        message: format!(
            "Template '{}' updated successfully with {} change(s)",
            name,
            changes.len()
        ),
        changes,
    };

    Ok(CommandResult::text(output).with_title(format!("Edit Template: {}", name)))
}

fn prompt_template_selection(config: &TemplatesConfig) -> Result<String> {
    let mut names: Vec<&String> = config.templates.keys().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No templates configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select template to edit")
        .items(&names)
        .default(0)
        .interact()
        .context("Failed to get template selection")?;

    Ok(names[selection].clone())
}

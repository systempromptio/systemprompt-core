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
use super::super::types::{TemplateEditOutput, TemplateEntry, TemplatesConfig};
use super::selection::prompt_template_selection;

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

pub(super) fn execute(
    args: EditArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    execute_in_dir(args, prompter, config, &WebPaths::resolve()?.templates)
}

pub fn execute_in_dir(
    args: EditArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
    templates_dir: &Path,
) -> Result<CommandOutput> {
    let templates_yaml_path = templates_dir.join("templates.yaml");

    let mut templates_config = load_templates_config(&templates_yaml_path)?;

    let EditArgs {
        name,
        add_content_type,
        remove_content_type,
        content,
        content_types,
    } = args;

    let name = resolve_required(name, "name", config, || {
        prompt_template_selection(prompter, &templates_config, "Select template to edit")
    })?;

    let entry = templates_config
        .templates
        .get_mut(&name)
        .ok_or_else(|| anyhow!("Template '{}' not found", name))?;

    let edits = ContentTypeEdits {
        set: content_types,
        add: add_content_type,
        remove: remove_content_type,
    };
    let mut changes = apply_content_type_edits(entry, edits)?;

    if let Some(content_source) = &content {
        let html_content = read_html_content(content_source)?;
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

    save_templates_config(&templates_yaml_path, &templates_config)?;

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

    Ok(CommandOutput::card_value(
        format!("Edit Template: {}", name),
        &output,
    ))
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

struct ContentTypeEdits {
    set: Option<String>,
    add: Option<String>,
    remove: Option<String>,
}

fn apply_content_type_edits(
    entry: &mut TemplateEntry,
    edits: ContentTypeEdits,
) -> Result<Vec<String>> {
    let mut changes = Vec::new();

    if let Some(ct) = edits.set {
        let new_types: Vec<String> = ct.split(',').map(|s| s.trim().to_owned()).collect();
        entry.content_types.clone_from(&new_types);
        changes.push(format!("content_types: {:?}", new_types));
    }

    if let Some(add_type) = edits.add {
        if entry.content_types.contains(&add_type) {
            CliService::warning(&format!(
                "Content type '{}' already linked to template",
                add_type
            ));
        } else {
            entry.content_types.push(add_type.clone());
            changes.push(format!("added content_type: {}", add_type));
        }
    }

    if let Some(remove_type) = edits.remove {
        if let Some(pos) = entry.content_types.iter().position(|x| *x == remove_type) {
            entry.content_types.remove(pos);
            changes.push(format!("removed content_type: {}", remove_type));
        } else {
            return Err(anyhow!(
                "Content type '{}' not linked to template",
                remove_type
            ));
        }
    }

    Ok(changes)
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

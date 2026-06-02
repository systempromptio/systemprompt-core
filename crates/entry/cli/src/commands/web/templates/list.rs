use anyhow::{Context, Result};
use clap::Args;
use std::fs;

use crate::CliConfig;
use crate::shared::CommandOutput;

use super::super::paths::WebPaths;
use super::super::types::{TemplateSummary, TemplatesConfig};

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only templates with missing files")]
    pub missing: bool,
}

pub(super) fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let web_paths = WebPaths::resolve()?;
    let templates_dir = &web_paths.templates;
    let templates_yaml_path = templates_dir.join("templates.yaml");

    if !templates_yaml_path.exists() {
        let empty: Vec<TemplateSummary> = vec![];
        return Ok(CommandOutput::table_of(
            vec!["name", "content_types", "file_exists", "file_path"],
            &empty,
        )
        .with_title("Templates"));
    }

    let content = fs::read_to_string(&templates_yaml_path).with_context(|| {
        format!(
            "Failed to read templates config at {}",
            templates_yaml_path.display()
        )
    })?;

    let config: TemplatesConfig = serde_yaml::from_str(&content).with_context(|| {
        format!(
            "Failed to parse templates config at {}",
            templates_yaml_path.display()
        )
    })?;

    let mut templates: Vec<TemplateSummary> = config
        .templates
        .iter()
        .map(|(name, entry)| {
            let file_path = templates_dir.join(format!("{}.html", name));
            let file_exists = file_path.exists();

            TemplateSummary {
                name: name.clone(),
                content_types: entry.content_types.clone(),
                file_exists,
                file_path: file_path.to_string_lossy().to_string(),
            }
        })
        .filter(|t| !args.missing || !t.file_exists)
        .collect();

    templates.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(CommandOutput::table_of(
        vec!["name", "content_types", "file_exists", "file_path"],
        &templates,
    )
    .with_title("Templates"))
}

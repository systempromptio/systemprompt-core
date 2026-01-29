use anyhow::{Context, Result};
use clap::Args;
use std::fs;

use crate::shared::CommandResult;
use crate::CliConfig;

use super::super::paths::WebPaths;
use super::super::types::{TemplateListOutput, TemplateSummary, TemplatesConfig};

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only templates with missing files")]
    pub missing: bool,
}

pub fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<TemplateListOutput>> {
    let web_paths = WebPaths::resolve()?;
    let templates_dir = &web_paths.templates;
    let templates_yaml_path = templates_dir.join("templates.yaml");

    if !templates_yaml_path.exists() {
        return Ok(
            CommandResult::table(TemplateListOutput { templates: vec![] })
                .with_title("Templates")
                .with_columns(vec![
                    "name".to_string(),
                    "content_types".to_string(),
                    "file_exists".to_string(),
                    "file_path".to_string(),
                ]),
        );
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

    let output = TemplateListOutput { templates };

    Ok(CommandResult::table(output)
        .with_title("Templates")
        .with_columns(vec![
            "name".to_string(),
            "content_types".to_string(),
            "file_exists".to_string(),
            "file_path".to_string(),
        ]))
}

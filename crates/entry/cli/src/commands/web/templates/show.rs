use anyhow::{Context, Result, anyhow};
use clap::Args;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::CliConfig;
use crate::interactive::{Prompter, resolve_required};
use crate::shared::CommandOutput;

use super::super::paths::WebPaths;
use super::super::types::{TemplateDetailOutput, TemplatesConfig};
use super::selection::prompt_template_selection;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Template name")]
    pub name: Option<String>,

    #[arg(long, help = "Number of preview lines", default_value = "20")]
    pub preview_lines: usize,
}

pub(super) fn execute(
    args: ShowArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    execute_in_dir(args, prompter, config, &WebPaths::resolve()?.templates)
}

pub fn execute_in_dir(
    args: ShowArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
    templates_dir: &Path,
) -> Result<CommandOutput> {
    let templates_yaml_path = templates_dir.join("templates.yaml");

    let content = fs::read_to_string(&templates_yaml_path).with_context(|| {
        format!(
            "Failed to read templates config at {}",
            templates_yaml_path.display()
        )
    })?;

    let templates_config: TemplatesConfig = serde_yaml::from_str(&content).with_context(|| {
        format!(
            "Failed to parse templates config at {}",
            templates_yaml_path.display()
        )
    })?;

    let name = resolve_required(args.name, "name", config, || {
        prompt_template_selection(prompter, &templates_config, "Select template")
    })?;

    let entry = templates_config
        .templates
        .get(&name)
        .ok_or_else(|| anyhow!("Template '{}' not found", name))?;

    let file_path = templates_dir.join(format!("{}.html", name));
    let file_exists = file_path.exists();

    let (variables, preview_lines) = if file_exists {
        let file_content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read template file at {}", file_path.display()))?;

        let variables = extract_template_variables(&file_content);

        let file = fs::File::open(&file_path)
            .with_context(|| format!("Failed to open template file at {}", file_path.display()))?;
        let reader = BufReader::new(file);
        let preview: Vec<String> = reader
            .lines()
            .take(args.preview_lines)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to read preview lines")?;

        (variables, preview)
    } else {
        (vec![], vec![])
    };

    let output = TemplateDetailOutput {
        name: name.clone(),
        content_types: entry.content_types.clone(),
        file_path: file_path.to_string_lossy().to_string(),
        file_exists,
        variables,
        preview_lines,
    };

    Ok(CommandOutput::card_value(
        format!("Template: {}", name),
        &output,
    ))
}

fn extract_template_variables(content: &str) -> Vec<String> {
    let Ok(re) = Regex::new(r"\{\{([A-Z_]+)\}\}") else {
        return vec![];
    };
    let mut variables: HashSet<String> = HashSet::new();

    for cap in re.captures_iter(content) {
        if let Some(matched) = cap.get(1) {
            variables.insert(matched.as_str().to_owned());
        }
    }

    let mut sorted: Vec<String> = variables.into_iter().collect();
    sorted.sort();
    sorted
}

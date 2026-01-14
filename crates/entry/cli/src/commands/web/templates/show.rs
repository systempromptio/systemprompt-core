use anyhow::{anyhow, Context, Result};
use clap::Args;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::{TemplateDetailOutput, TemplatesConfig};

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Template name")]
    pub name: Option<String>,

    #[arg(long, help = "Number of preview lines", default_value = "20")]
    pub preview_lines: usize,
}

pub fn execute(args: ShowArgs, config: &CliConfig) -> Result<CommandResult<TemplateDetailOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let web_path = profile.paths.web_path_resolved();
    let templates_dir = Path::new(&web_path).join("templates");
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

    let name = resolve_input(args.name, "name", config, || {
        prompt_template_selection(&templates_config)
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

        let file = fs::File::open(&file_path)?;
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

    Ok(CommandResult::card(output).with_title(format!("Template: {}", name)))
}

fn extract_template_variables(content: &str) -> Vec<String> {
    let re = Regex::new(r"\{\{([A-Z_]+)\}\}").expect("Invalid regex");
    let mut variables: HashSet<String> = HashSet::new();

    for cap in re.captures_iter(content) {
        if let Some(matched) = cap.get(1) {
            variables.insert(matched.as_str().to_string());
        }
    }

    let mut sorted: Vec<String> = variables.into_iter().collect();
    sorted.sort();
    sorted
}

fn prompt_template_selection(config: &TemplatesConfig) -> Result<String> {
    let mut names: Vec<&String> = config.templates.keys().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No templates configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select template")
        .items(&names)
        .default(0)
        .interact()
        .context("Failed to get template selection")?;

    Ok(names[selection].clone())
}

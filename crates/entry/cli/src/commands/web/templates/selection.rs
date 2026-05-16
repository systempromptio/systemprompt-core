use anyhow::{Context, Result, anyhow};
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;

use super::super::types::TemplatesConfig;

pub fn prompt_template_selection(config: &TemplatesConfig, prompt: &str) -> Result<String> {
    let mut names: Vec<&String> = config.templates.keys().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No templates configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(&names)
        .default(0)
        .interact()
        .context("Failed to get template selection")?;

    Ok(names[selection].clone())
}

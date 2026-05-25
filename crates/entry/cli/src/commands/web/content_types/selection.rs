use anyhow::{Context, Result, anyhow};
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use systemprompt_models::content_config::ContentConfigRaw;

pub(crate) fn prompt_content_type_selection(config: &ContentConfigRaw, prompt: &str) -> Result<String> {
    let mut names: Vec<&String> = config.content_sources.keys().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No content types configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(&names)
        .default(0)
        .interact()
        .context("Failed to get content type selection")?;

    Ok(names[selection].clone())
}

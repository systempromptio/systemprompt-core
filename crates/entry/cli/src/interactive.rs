use crate::CliConfig;
use anyhow::{anyhow, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Select};

pub fn require_confirmation(
    message: &str,
    skip_confirmation: bool,
    config: &CliConfig,
) -> Result<()> {
    if skip_confirmation {
        return Ok(());
    }

    if !config.is_interactive() {
        return Err(anyhow!("--yes is required in non-interactive mode"));
    }

    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .default(false)
        .interact()?;

    if confirmed {
        Ok(())
    } else {
        Err(anyhow!("Operation cancelled"))
    }
}

pub fn require_confirmation_default_yes(
    message: &str,
    skip_confirmation: bool,
    config: &CliConfig,
) -> Result<()> {
    if skip_confirmation {
        return Ok(());
    }

    if !config.is_interactive() {
        return Err(anyhow!("--yes is required in non-interactive mode"));
    }

    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .default(true)
        .interact()?;

    if confirmed {
        Ok(())
    } else {
        Err(anyhow!("Operation cancelled"))
    }
}

pub fn resolve_required<T, F>(
    value: Option<T>,
    flag_name: &str,
    config: &CliConfig,
    prompt_fn: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    match value {
        Some(v) => Ok(v),
        None if config.is_interactive() => prompt_fn(),
        None => Err(anyhow!(
            "--{} is required in non-interactive mode",
            flag_name
        )),
    }
}

pub fn select_from_list<T: ToString + Clone>(
    prompt: &str,
    items: &[T],
    flag_name: &str,
    config: &CliConfig,
) -> Result<T> {
    if items.is_empty() {
        return Err(anyhow!("No items available for selection"));
    }

    if !config.is_interactive() {
        return Err(anyhow!(
            "--{} is required in non-interactive mode",
            flag_name
        ));
    }

    let display: Vec<String> = items.iter().map(ToString::to_string).collect();

    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(&display)
        .default(0)
        .interact()?;

    Ok(items[idx].clone())
}

pub fn select_index(prompt: &str, items: &[&str], config: &CliConfig) -> Result<Option<usize>> {
    if !config.is_interactive() {
        return Ok(None);
    }

    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(items)
        .default(0)
        .interact()?;

    Ok(Some(idx))
}

pub fn prompt_input(prompt: &str, flag_name: &str, config: &CliConfig) -> Result<String> {
    if !config.is_interactive() {
        return Err(anyhow!(
            "--{} is required in non-interactive mode",
            flag_name
        ));
    }

    let input = dialoguer::Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact_text()?;

    Ok(input)
}

pub fn prompt_input_with_default(
    prompt: &str,
    default: &str,
    config: &CliConfig,
) -> Result<String> {
    if !config.is_interactive() {
        return Ok(default.to_string());
    }

    let input = dialoguer::Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(default.to_string())
        .interact_text()?;

    Ok(input)
}

pub fn confirm_optional(message: &str, default: bool, config: &CliConfig) -> Result<bool> {
    if !config.is_interactive() {
        return Ok(default);
    }

    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .default(default)
        .interact()?;

    Ok(confirmed)
}

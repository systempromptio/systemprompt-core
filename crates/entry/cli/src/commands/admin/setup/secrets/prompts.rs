//! Interactive provider-key collection for the setup wizard.
//!
//! These run only on the interactive path; the non-interactive collector reads
//! keys straight from flags. [`select_provider_keys`] returns the explicit
//! default when the user picks a single provider, and `None` when they enter
//! several keys (the default is then resolved by [`resolve_interactive_primary`]).

use anyhow::{Result, anyhow};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Password, Select};
use systemprompt_identifiers::ProviderId;
use systemprompt_logging::CliService;

use super::SecretsData;

/// Returns `None` for the "enter multiple keys" path; that default is resolved
/// later from the keys actually present, not at selection time.
pub(super) fn select_provider_keys(secrets: &mut SecretsData) -> Result<Option<ProviderId>> {
    let providers = vec![
        "Google AI (Gemini) - https://aistudio.google.com/app/apikey",
        "Anthropic (Claude) - https://console.anthropic.com/api-keys",
        "OpenAI (GPT) - https://platform.openai.com/api-keys",
        "Enter multiple keys",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select your AI provider")
        .items(&providers)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            secrets.gemini = Some(prompt_api_key("Gemini API Key")?);
            Ok(Some(ProviderId::new("gemini")))
        },
        1 => {
            secrets.anthropic = Some(prompt_api_key("Anthropic API Key")?);
            Ok(Some(ProviderId::new("anthropic")))
        },
        2 => {
            secrets.openai = Some(prompt_api_key("OpenAI API Key")?);
            Ok(Some(ProviderId::new("openai")))
        },
        3 => {
            CliService::info("Enter API keys (press Enter to skip any):");
            if let Some(key) = prompt_optional_api_key("Gemini API Key")? {
                secrets.gemini = Some(key);
            }
            if let Some(key) = prompt_optional_api_key("Anthropic API Key")? {
                secrets.anthropic = Some(key);
            }
            if let Some(key) = prompt_optional_api_key("OpenAI API Key")? {
                secrets.openai = Some(key);
            }
            if let Some(key) = prompt_optional_api_key("GitHub Token (optional)")? {
                secrets.github = Some(key);
            }
            Ok(None)
        },
        _ => Err(anyhow!("Invalid AI provider option selected")),
    }
}

/// Decide the default provider after an interactive collection: the explicit
/// single-select wins; otherwise the sole present key, or a follow-up prompt
/// when several keys were entered.
pub(super) fn resolve_interactive_primary(
    explicit: Option<ProviderId>,
    secrets: &SecretsData,
) -> Result<Option<ProviderId>> {
    if explicit.is_some() {
        return Ok(explicit);
    }
    match secrets.present_providers().as_slice() {
        [] => Ok(None),
        [only] => Ok(Some(ProviderId::new(*only))),
        present => {
            let idx = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Which provider should be the default?")
                .items(present)
                .default(0)
                .interact()?;
            Ok(Some(ProviderId::new(present[idx])))
        },
    }
}

fn prompt_api_key(prompt: &str) -> Result<String> {
    let key = Password::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact()?;

    if key.is_empty() {
        anyhow::bail!("API key is required");
    }

    Ok(key)
}

fn prompt_optional_api_key(prompt: &str) -> Result<Option<String>> {
    let key = Password::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .allow_empty_password(true)
        .interact()?;

    if key.is_empty() {
        Ok(None)
    } else {
        Ok(Some(key))
    }
}

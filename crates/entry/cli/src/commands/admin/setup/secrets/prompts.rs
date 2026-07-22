//! Interactive provider-key collection for the setup wizard.
//!
//! These run only on the interactive path; the non-interactive collector reads
//! keys straight from flags. [`select_provider_keys`] returns the explicit
//! default when the user picks a single provider, and `None` when they enter
//! several keys (the default is then resolved by
//! [`resolve_interactive_primary`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use systemprompt_identifiers::ProviderId;
use systemprompt_logging::CliService;

use super::SecretsData;
use crate::interactive::Prompter;

/// Returns `None` for the "enter multiple keys" path; that default is resolved
/// later from the keys actually present, not at selection time.
pub fn select_provider_keys(
    prompter: &dyn Prompter,
    secrets: &mut SecretsData,
) -> Result<Option<ProviderId>> {
    let providers = vec![
        "Google AI (Gemini) - https://aistudio.google.com/app/apikey".to_owned(),
        "Anthropic (Claude) - https://console.anthropic.com/api-keys".to_owned(),
        "OpenAI (GPT) - https://platform.openai.com/api-keys".to_owned(),
        "Enter multiple keys".to_owned(),
    ];

    let selection = prompter.select("Select your AI provider", &providers)?;

    match selection {
        0 => {
            secrets.gemini = Some(prompt_api_key(prompter, "Gemini API Key")?);
            Ok(Some(ProviderId::new("gemini")))
        },
        1 => {
            secrets.anthropic = Some(prompt_api_key(prompter, "Anthropic API Key")?);
            Ok(Some(ProviderId::new("anthropic")))
        },
        2 => {
            secrets.openai = Some(prompt_api_key(prompter, "OpenAI API Key")?);
            Ok(Some(ProviderId::new("openai")))
        },
        3 => {
            CliService::info("Enter API keys (press Enter to skip any):");
            if let Some(key) = prompt_optional_api_key(prompter, "Gemini API Key")? {
                secrets.gemini = Some(key);
            }
            if let Some(key) = prompt_optional_api_key(prompter, "Anthropic API Key")? {
                secrets.anthropic = Some(key);
            }
            if let Some(key) = prompt_optional_api_key(prompter, "OpenAI API Key")? {
                secrets.openai = Some(key);
            }
            if let Some(key) = prompt_optional_api_key(prompter, "GitHub Token (optional)")? {
                secrets.github = Some(key);
            }
            Ok(None)
        },
        _ => Err(anyhow!("Invalid AI provider option selected")),
    }
}

pub fn resolve_interactive_primary(
    prompter: &dyn Prompter,
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
            let items: Vec<String> = present.iter().map(|p| (*p).to_owned()).collect();
            let idx = prompter.select("Which provider should be the default?", &items)?;
            Ok(Some(ProviderId::new(present[idx])))
        },
    }
}

fn prompt_api_key(prompter: &dyn Prompter, prompt: &str) -> Result<String> {
    let key = prompter.password(prompt)?;

    if key.is_empty() {
        anyhow::bail!("API key is required");
    }

    Ok(key)
}

fn prompt_optional_api_key(prompter: &dyn Prompter, prompt: &str) -> Result<Option<String>> {
    let key = prompter.password(prompt)?;

    if key.is_empty() {
        Ok(None)
    } else {
        Ok(Some(key))
    }
}

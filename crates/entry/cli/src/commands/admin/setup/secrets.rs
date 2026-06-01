//! Secret collection and persistence for the setup wizard.
//!
//! [`SecretsData`] holds the generated OAuth at-rest pepper, database URL, and
//! AI-provider keys. The `collect_*` functions gather these interactively or
//! from flags, [`validate_secrets`] enforces that at least one provider key is
//! present, and [`save`] writes the file with `0600` permissions on Unix.

use anyhow::{Context, Result, anyhow, bail};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Password, Select};
use serde::{Deserialize, Serialize};
use std::path::Path;
use systemprompt_identifiers::ProviderId;
use systemprompt_logging::CliService;

use super::SetupArgs;
use crate::CliConfig;
use crate::shared::profile::generate_oauth_at_rest_pepper;

const STANDARD_PROVIDERS: [&str; 3] = ["gemini", "anthropic", "openai"];

// Default-provider precedence when no --default-provider flag is given: the
// first provider in this order whose key was supplied wins.
const PROVIDER_PRIORITY: [&str; 3] = ["anthropic", "openai", "gemini"];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct SecretsData {
    pub oauth_at_rest_pepper: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
}

impl SecretsData {
    pub(super) const fn has_ai_provider(&self) -> bool {
        self.gemini.is_some() || self.anthropic.is_some() || self.openai.is_some()
    }

    fn key_for(&self, provider: &str) -> Option<&String> {
        match provider {
            "gemini" => self.gemini.as_ref(),
            "anthropic" => self.anthropic.as_ref(),
            "openai" => self.openai.as_ref(),
            _ => None,
        }
    }

    pub(super) fn present_providers(&self) -> Vec<&'static str> {
        STANDARD_PROVIDERS
            .into_iter()
            .filter(|p| self.key_for(p).is_some())
            .collect()
    }

    pub(super) fn summary(&self) -> String {
        let mut keys = Vec::new();
        if self.gemini.is_some() {
            keys.push("Gemini");
        }
        if self.anthropic.is_some() {
            keys.push("Anthropic");
        }
        if self.openai.is_some() {
            keys.push("OpenAI");
        }
        if self.github.is_some() {
            keys.push("GitHub");
        }

        if keys.is_empty() {
            "None".to_owned()
        } else {
            keys.join(", ")
        }
    }
}

fn first_present_by_priority(secrets: &SecretsData) -> Option<ProviderId> {
    PROVIDER_PRIORITY
        .into_iter()
        .find(|p| secrets.key_for(p).is_some())
        .map(ProviderId::new)
}

/// An explicit, key-backed `--default-provider` flag wins; otherwise the first
/// present key by [`PROVIDER_PRIORITY`]. The flag's absence is never fatal —
/// `validate_secrets` already guarantees at least one key is present.
fn resolve_primary(args: &SetupArgs, secrets: &SecretsData) -> Result<Option<ProviderId>> {
    let Some(name) = args.default_provider.as_deref().map(str::trim) else {
        return Ok(first_present_by_priority(secrets));
    };
    if !STANDARD_PROVIDERS.contains(&name) {
        bail!("--default-provider must be one of: gemini, anthropic, openai (got '{name}')");
    }
    if secrets.key_for(name).is_none() {
        bail!(
            "--default-provider '{name}' has no API key; pass --{name}-key or drop \
             --default-provider"
        );
    }
    Ok(Some(ProviderId::new(name)))
}

pub(super) fn collect_non_interactive(
    args: &SetupArgs,
    config: &CliConfig,
) -> Result<(SecretsData, Option<ProviderId>)> {
    if !config.is_json_output() {
        CliService::section("Secrets Setup");
    }

    let oauth_at_rest_pepper = generate_oauth_at_rest_pepper();
    if !config.is_json_output() {
        CliService::success("Generated secure OAuth at-rest pepper (64 characters)");
    }

    let secrets = SecretsData {
        oauth_at_rest_pepper,
        database_url: None,
        gemini: args.gemini_key.clone(),
        anthropic: args.anthropic_key.clone(),
        openai: args.openai_key.clone(),
        github: args.github_token.clone(),
    };

    validate_secrets(&secrets)?;
    let primary = resolve_primary(args, &secrets)?;

    if !config.is_json_output() {
        CliService::success(&format!("Configured keys: {}", secrets.summary()));
    }

    Ok((secrets, primary))
}

pub(super) fn collect_interactive(
    args: &SetupArgs,
    env_name: &str,
    _config: &CliConfig,
) -> Result<(SecretsData, Option<ProviderId>)> {
    CliService::section(&format!("Secrets Setup ({})", env_name));
    CliService::info("At least one AI provider API key is required.");

    let oauth_at_rest_pepper = generate_oauth_at_rest_pepper();
    CliService::success("Generated secure OAuth at-rest pepper (64 characters)");

    let mut secrets = SecretsData {
        oauth_at_rest_pepper,
        ..Default::default()
    };

    if args.has_ai_provider() {
        args.gemini_key.clone_into(&mut secrets.gemini);
        args.anthropic_key.clone_into(&mut secrets.anthropic);
        args.openai_key.clone_into(&mut secrets.openai);
        args.github_token.clone_into(&mut secrets.github);
        CliService::success(&format!("Using provided keys: {}", secrets.summary()));
        let primary = resolve_primary(args, &secrets)?;
        return Ok((secrets, primary));
    }

    let explicit = select_provider_keys(&mut secrets)?;
    validate_secrets(&secrets)?;
    let primary = resolve_interactive_primary(explicit, &secrets)?;

    CliService::success(&format!("Configured keys: {}", secrets.summary()));

    Ok((secrets, primary))
}

/// Returns `None` for the "enter multiple keys" path; that default is resolved
/// later from the keys actually present, not at selection time.
fn select_provider_keys(secrets: &mut SecretsData) -> Result<Option<ProviderId>> {
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
fn resolve_interactive_primary(
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

fn validate_secrets(secrets: &SecretsData) -> Result<()> {
    if secrets.oauth_at_rest_pepper.len() < 32 {
        anyhow::bail!("OAuth at-rest pepper must be at least 32 characters");
    }

    if !secrets.has_ai_provider() {
        anyhow::bail!(
            "At least one AI provider API key is required.\n\n\
             Provide one of:\n\
             --gemini-key <KEY>     Google AI (Gemini)\n\
             --anthropic-key <KEY>  Anthropic (Claude)\n\
             --openai-key <KEY>     OpenAI (GPT)\n\n\
             Or set environment variables:\n\
             GEMINI_API_KEY, ANTHROPIC_API_KEY, or OPENAI_API_KEY"
        );
    }

    Ok(())
}

pub(super) fn save(secrets: &SecretsData, secrets_path: &Path) -> Result<()> {
    if let Some(parent) = secrets_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(secrets).context("Failed to serialize secrets")?;

    std::fs::write(secrets_path, content)
        .with_context(|| format!("Failed to write {}", secrets_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(secrets_path, permissions)
            .with_context(|| format!("Failed to set permissions on {}", secrets_path.display()))?;
    }

    CliService::success(&format!("Saved secrets to {}", secrets_path.display()));

    Ok(())
}

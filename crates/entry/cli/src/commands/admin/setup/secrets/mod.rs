//! Secret collection and persistence for the setup wizard.
//!
//! The `collect_*` functions gather the OAuth at-rest pepper, database URL, and
//! AI-provider keys interactively or from flags; [`save`] writes the file with
//! `0600` permissions on Unix. The data model and default-provider resolution
//! live in [`data`]; the interactive prompts live in [`prompts`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod data;
mod prompts;

use anyhow::{Context, Result};
use std::path::Path;
use systemprompt_identifiers::ProviderId;
use systemprompt_logging::CliService;

use super::SetupArgs;
use crate::CliConfig;
use crate::interactive::Prompter;
use crate::shared::profile::generate_oauth_at_rest_pepper;
use data::resolve_primary;
use prompts::{resolve_interactive_primary, select_provider_keys};

pub use data::SecretsData;

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
    prompter: &dyn Prompter,
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

    let explicit = select_provider_keys(prompter, &mut secrets)?;
    validate_secrets(&secrets)?;
    let primary = resolve_interactive_primary(prompter, explicit, &secrets)?;

    CliService::success(&format!("Configured keys: {}", secrets.summary()));

    Ok((secrets, primary))
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

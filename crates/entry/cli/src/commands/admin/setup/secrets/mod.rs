//! Secret collection and persistence for the setup wizard.
//!
//! The `collect_*` functions gather the OAuth at-rest pepper, database URL, and
//! AI-provider keys interactively or from flags; `save` writes the file with
//! `0600` permissions on Unix. The data model and default-provider resolution
//! live in `data`; the interactive prompts live in [`prompts`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod data;
pub mod prompts;
pub mod vertex;

use anyhow::{Context, Result};
use std::path::Path;
use systemprompt_config::write_private_atomic;
use systemprompt_identifiers::ProviderId;
use systemprompt_logging::CliService;

use super::SetupArgs;
use data::resolve_primary;
use prompts::{resolve_interactive_primary, select_provider_keys};
use systemprompt_cloud::identity::generate_identity;
use systemprompt_extension::cli::{CliConfig, Prompter};

pub use data::SecretsData;

/// Whether the setup wizard must collect an AI provider key: required exactly
/// when the composition admits the `ai` member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderRequirement {
    Required,
    Optional,
}

impl ProviderRequirement {
    #[must_use]
    pub fn for_composition(built: &systemprompt_extension::BuiltState) -> Self {
        if built.contains(&systemprompt_identifiers::ExtensionId::new("ai")) {
            Self::Required
        } else {
            Self::Optional
        }
    }
}

#[doc(hidden)]
pub fn collect_non_interactive(
    args: &SetupArgs,
    config: &CliConfig,
    providers: ProviderRequirement,
) -> Result<(SecretsData, Option<ProviderId>)> {
    if !config.is_json_output() {
        CliService::section("Secrets Setup");
    }

    let identity = generate_identity()?;
    if !config.is_json_output() {
        CliService::success(
            "Generated OAuth at-rest pepper, encryption master key, manifest signing seed and signing key",
        );
    }

    let mut secrets = SecretsData {
        oauth_at_rest_pepper: identity.oauth_at_rest_pepper,
        encryption_master_key: identity.encryption_master_key,
        manifest_signing_secret_seed: Some(identity.manifest_signing_secret_seed),
        signing_key_pem: Some(identity.signing_key_pem),
        gemini: args.gemini_key.clone(),
        anthropic: args.anthropic_key.clone(),
        openai: args.openai_key.clone(),
        github: args.github_token.clone(),
        ..SecretsData::default()
    };
    vertex::apply(
        &mut secrets,
        args.vertex_project.as_deref(),
        args.vertex_key_file.as_deref(),
    )?;

    validate_secrets(&secrets, providers)?;
    let primary = resolve_primary(args, &secrets)?;

    if !config.is_json_output() {
        CliService::success(&format!("Configured keys: {}", secrets.summary()));
    }

    Ok((secrets, primary))
}

#[doc(hidden)]
pub fn collect_interactive(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    env_name: &str,
    _config: &CliConfig,
    providers: ProviderRequirement,
) -> Result<(SecretsData, Option<ProviderId>)> {
    CliService::section(&format!("Secrets Setup ({})", env_name));
    if providers == ProviderRequirement::Required {
        CliService::info("At least one AI provider is required: an API key or Vertex AI.");
    }

    let identity = generate_identity()?;
    CliService::success(
        "Generated OAuth at-rest pepper, encryption master key, manifest signing seed and signing key",
    );

    let mut secrets = SecretsData {
        oauth_at_rest_pepper: identity.oauth_at_rest_pepper,
        encryption_master_key: identity.encryption_master_key,
        manifest_signing_secret_seed: Some(identity.manifest_signing_secret_seed),
        signing_key_pem: Some(identity.signing_key_pem),
        ..Default::default()
    };

    if args.has_ai_provider() {
        args.gemini_key.clone_into(&mut secrets.gemini);
        args.anthropic_key.clone_into(&mut secrets.anthropic);
        args.openai_key.clone_into(&mut secrets.openai);
        args.github_token.clone_into(&mut secrets.github);
        vertex::apply(
            &mut secrets,
            args.vertex_project.as_deref(),
            args.vertex_key_file.as_deref(),
        )?;
        CliService::success(&format!("Using provided keys: {}", secrets.summary()));
        let primary = resolve_primary(args, &secrets)?;
        return Ok((secrets, primary));
    }

    let explicit = match providers {
        ProviderRequirement::Required => select_provider_keys(prompter, &mut secrets)?,
        ProviderRequirement::Optional => None,
    };
    validate_secrets(&secrets, providers)?;
    let primary = resolve_interactive_primary(prompter, explicit, &secrets)?;

    CliService::success(&format!("Configured keys: {}", secrets.summary()));

    Ok((secrets, primary))
}

fn validate_secrets(secrets: &SecretsData, providers: ProviderRequirement) -> Result<()> {
    if secrets.oauth_at_rest_pepper.len() < 32 {
        anyhow::bail!("OAuth at-rest pepper must be at least 32 characters");
    }
    systemprompt_config::decode_master_key(&secrets.encryption_master_key)?;

    if providers == ProviderRequirement::Required && !secrets.has_ai_provider() {
        anyhow::bail!(
            "At least one AI provider is required.\n\n\
             Provide one of:\n\
             --gemini-key <KEY>          Google AI (Gemini)\n\
             --anthropic-key <KEY>       Anthropic (Claude)\n\
             --openai-key <KEY>          OpenAI (GPT)\n\
             --vertex-project <ID>       Vertex AI with the runtime's workload identity\n\
             --vertex-key-file <PATH>    Vertex AI with a service-account key file\n\n\
             Or set environment variables:\n\
             GEMINI_API_KEY, ANTHROPIC_API_KEY, OPENAI_API_KEY,\n\
             SYSTEMPROMPT_VERTEX_PROJECT or SYSTEMPROMPT_VERTEX_KEY_FILE"
        );
    }

    Ok(())
}

pub(super) fn save(secrets: &SecretsData, secrets_path: &Path) -> Result<()> {
    let content = CliService::json_string(secrets).context("Failed to serialize secrets")?;
    if let Some(parent) = secrets_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    write_private_atomic(secrets_path, content.as_bytes())
        .with_context(|| format!("Failed to write {}", secrets_path.display()))?;

    CliService::success(&format!("Saved secrets to {}", secrets_path.display()));

    Ok(())
}

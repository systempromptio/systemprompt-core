//! `admin config secret set` — write a provider or custom credential into the
//! profile's secrets file without hand-editing JSON.
//!
//! Infrastructure secrets (database URLs, at-rest pepper, signing seed) are
//! rejected: they are provisioned out-of-band, so a partial edit here cannot
//! corrupt the values the runtime depends on.

use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;

use super::profile_io::{load_profile, profile_dir};
use super::types::ConfigMutationOutput;
use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

const RESERVED: &[&str] = &[
    "oauth_at_rest_pepper",
    "manifest_signing_secret_seed",
    "database_url",
    "database_write_url",
    "external_database_url",
    "internal_database_url",
];

#[derive(Debug, Subcommand)]
pub enum SecretCommands {
    #[command(about = "Set a provider or custom secret")]
    Set(SetArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(help = "Secret name (e.g. anthropic, minimax)")]
    pub name: String,

    #[arg(help = "Secret value")]
    pub value: String,
}

pub fn execute(command: &SecretCommands, config: &CliConfig) -> Result<()> {
    let SecretCommands::Set(args) = command;

    let profile_path = ProfileBootstrap::get_path()?;
    let profile = load_profile(profile_path)?;
    let secrets_rel = profile
        .secrets
        .as_ref()
        .map(|s| s.secrets_path.clone())
        .ok_or_else(|| anyhow::anyhow!("profile has no secrets section"))?;
    let secrets_file = profile_dir(profile_path).join(&secrets_rel);

    set_secret(&secrets_file, &args.name, &args.value)?;

    render_result(
        &CommandOutput::card_value(
            "Secret Updated",
            &ConfigMutationOutput {
                field: "secrets".to_owned(),
                message: format!("Secret '{}' set", args.name),
            },
        ),
        config,
    );
    Ok(())
}

pub fn set_secret(secrets_file: &Path, name: &str, value: &str) -> Result<()> {
    if RESERVED.contains(&name) {
        bail!("'{name}' is a reserved infrastructure secret and cannot be set here");
    }

    let content = std::fs::read_to_string(secrets_file)
        .with_context(|| format!("Failed to read secrets: {}", secrets_file.display()))?;
    // JSON: operator tooling edits the on-disk secrets document by key, so a
    // file still missing a required field can be completed one secret at a time.
    let mut doc: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse secrets: {}", secrets_file.display()))?;
    let object = doc.as_object_mut().ok_or_else(|| {
        anyhow::anyhow!(
            "secrets file is not a JSON object: {}",
            secrets_file.display()
        )
    })?;
    object.insert(name.to_owned(), serde_json::Value::String(value.to_owned()));

    let serialized = serde_json::to_string_pretty(&doc).context("Failed to serialize secrets")?;
    std::fs::write(secrets_file, serialized)
        .with_context(|| format!("Failed to write {}", secrets_file.display()))?;
    Ok(())
}

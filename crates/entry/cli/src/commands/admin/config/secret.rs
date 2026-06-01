//! `admin config secret set` — write a provider credential into the profile's
//! secrets file without hand-editing JSON.
//!
//! Known provider names map to their typed field; any other name becomes a
//! custom secret (e.g. `minimax`). Infrastructure secrets
//! (database/pepper/signing seed) are rejected so they cannot collide with the
//! typed fields on round-trip.

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;
use systemprompt_models::Secrets;

use super::profile_io::{load_profile, profile_dir};
use super::types::ConfigMutationOutput;
use crate::CliConfig;
use crate::shared::{CommandResult, render_result};

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

pub fn execute(command: &SecretCommands, _config: &CliConfig) -> Result<()> {
    let SecretCommands::Set(args) = command;

    if RESERVED.contains(&args.name.as_str()) {
        bail!(
            "'{}' is a reserved infrastructure secret and cannot be set here",
            args.name
        );
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let profile = load_profile(profile_path)?;
    let secrets_rel = profile
        .secrets
        .as_ref()
        .map(|s| s.secrets_path.clone())
        .ok_or_else(|| anyhow::anyhow!("profile has no secrets section"))?;
    let secrets_file = profile_dir(profile_path).join(&secrets_rel);

    let content = std::fs::read_to_string(&secrets_file)
        .with_context(|| format!("Failed to read secrets: {}", secrets_file.display()))?;
    let mut secrets: Secrets = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse secrets: {}", secrets_file.display()))?;

    set_named(&mut secrets, &args.name, args.value.clone());

    let serialized =
        serde_json::to_string_pretty(&secrets).context("Failed to serialize secrets")?;
    std::fs::write(&secrets_file, serialized)
        .with_context(|| format!("Failed to write {}", secrets_file.display()))?;

    render_result(
        &CommandResult::text(ConfigMutationOutput {
            field: "secrets".to_owned(),
            message: format!("Secret '{}' set", args.name),
        })
        .with_title("Secret Updated"),
    );
    Ok(())
}

fn set_named(secrets: &mut Secrets, name: &str, value: String) {
    match name {
        "gemini" => secrets.gemini = Some(value),
        "anthropic" => secrets.anthropic = Some(value),
        "openai" => secrets.openai = Some(value),
        "github" => secrets.github = Some(value),
        "moonshot" => secrets.moonshot = Some(value),
        "qwen" => secrets.qwen = Some(value),
        other => {
            secrets.custom.insert(other.to_owned(), value);
        },
    }
}

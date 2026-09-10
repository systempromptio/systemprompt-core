//! `admin config secret check` — report where secrets come from and which
//! keys are present.
//!
//! The command prints key *names* only. A value never reaches the terminal,
//! the JSON output, or a log line, because the usual reason to run this is a
//! failing boot on a shared screen.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use systemprompt_config::{ProfileBootstrap, ResolvedSource, SecretsProvider, VaultKvProvider};
use systemprompt_models::profile::resolve_with_home;
use systemprompt_models::secrets::OAUTH_AT_REST_PEPPER_MIN_LENGTH;

use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

pub const REQUIRED_SECRET_KEYS: &[&str] = &["oauth_at_rest_pepper", "database_url"];
pub const DATABASE_URL_ALIASES: &[&str] = &["database_url", "internal_database_url"];

#[derive(Debug, Serialize)]
pub struct SecretCheckReport {
    pub source: String,
    pub detail: String,
    pub keys: Vec<String>,
    pub missing_required: Vec<String>,
}

pub async fn execute(config: &CliConfig) -> Result<()> {
    let report = run().await?;
    render_result(
        &CommandOutput::card_value("Secrets Source", &report),
        config,
    );
    Ok(())
}

pub async fn run() -> Result<SecretCheckReport> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let profile_path = ProfileBootstrap::get_path().context("Failed to locate the profile")?;
    let profile_dir = Path::new(profile_path)
        .parent()
        .context("Profile path has no parent directory")?;

    let is_deployment_host =
        systemprompt_models::subprocess::is_deployment_host(|name| std::env::var(name).ok());
    let is_subprocess = std::env::var("SYSTEMPROMPT_SUBPROCESS").is_ok();
    let has_pepper = std::env::var("OAUTH_AT_REST_PEPPER")
        .is_ok_and(|pepper| pepper.len() >= OAUTH_AT_REST_PEPPER_MIN_LENGTH);

    let resolved = systemprompt_config::resolve_source(
        profile.secrets.as_ref(),
        is_subprocess,
        is_deployment_host,
        has_pepper,
    )
    .context("The profile's secrets configuration is not usable")?;

    let (source, detail, keys) = describe(&resolved, profile_dir).await?;
    Ok(SecretCheckReport {
        missing_required: missing_required(&keys),
        source,
        detail,
        keys,
    })
}

async fn describe(
    resolved: &ResolvedSource<'_>,
    profile_dir: &Path,
) -> Result<(String, String, Vec<String>)> {
    match resolved {
        ResolvedSource::Vault(cfg) => {
            let provider = VaultKvProvider::from_config(cfg, |name| std::env::var(name).ok())
                .context("Vault client could not be built from the profile")?;
            let detail = provider.describe();
            let document = provider
                .fetch()
                .await
                .context("Vault document could not be read")?;
            Ok(("vault".to_owned(), detail, document.key_names()))
        },
        ResolvedSource::SubprocessEnv => Ok((
            "subprocess-env".to_owned(),
            "inherited from the parent process".to_owned(),
            env_key_names(),
        )),
        ResolvedSource::DeploymentHostEnv => Ok((
            "deployment-host-env".to_owned(),
            "provided by the deployment host".to_owned(),
            env_key_names(),
        )),
        ResolvedSource::LocalEnvWithFileFallback(path) | ResolvedSource::File(path) => {
            let resolved_path = resolve_with_home(profile_dir, path);
            let keys = file_key_names(&resolved_path)?;
            Ok(("file".to_owned(), resolved_path.display().to_string(), keys))
        },
    }
}

pub fn file_key_names(path: &Path) -> Result<Vec<String>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    // JSON: the secrets document is operator-authored and read here only for
    // its key names; values are never touched.
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    let mut names: Vec<String> = parsed
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    Ok(names)
}

fn env_key_names() -> Vec<String> {
    let mut names: Vec<String> = REQUIRED_SECRET_KEYS
        .iter()
        .chain(DATABASE_URL_ALIASES.iter())
        .filter(|name| std::env::var(name.to_uppercase()).is_ok())
        .map(|name| (*name).to_owned())
        .collect();
    names.sort();
    names.dedup();
    names
}

#[must_use]
pub fn missing_required(keys: &[String]) -> Vec<String> {
    let present = |name: &str| keys.iter().any(|key| key.eq_ignore_ascii_case(name));
    let mut missing = Vec::new();
    for required in REQUIRED_SECRET_KEYS {
        if *required == "database_url" {
            if !DATABASE_URL_ALIASES.iter().any(|alias| present(alias)) {
                missing.push("database_url (or internal_database_url)".to_owned());
            }
        } else if !present(required) {
            missing.push((*required).to_owned());
        }
    }
    missing
}

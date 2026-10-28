//! Provisioning the deployed instance's secrets and bootstrap environment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, bail};
use systemprompt_cloud::constants::{container, paths};
use systemprompt_cloud::deploy::provider::{
    DeployProvider, SecretBatch, SecretPurpose, SecretsOutcome, SecretsTransport,
};
use systemprompt_cloud::{CloudCredentials, secrets_env};

use super::request::{DeployRequest, DeploySecretsSource};

const SIGNING_KEY_ENV: &str = "SIGNING_KEY_PEM";
const PROFILE_ENV: &str = "SYSTEMPROMPT_PROFILE";

pub(super) const fn pushed_count(outcome: &SecretsOutcome) -> usize {
    match outcome {
        SecretsOutcome::Pushed { keys } => keys.len(),
        SecretsOutcome::External { required } => required.len(),
    }
}

pub(super) async fn push<P: DeployProvider>(
    provider: &P,
    purpose: SecretPurpose,
    entries: HashMap<String, String>,
) -> Result<usize> {
    let outcome = provider
        .push_secrets(SecretBatch::Values { purpose, entries })
        .await?;
    Ok(pushed_count(&outcome))
}

pub(super) async fn provision_secrets<P: DeployProvider>(
    provider: &P,
    request: &DeployRequest,
    progress: &dyn DeployProgress,
) -> Result<()> {
    progress.event(&DeployEvent::SecretsPhaseStarted);

    if provider.secrets_transport() == SecretsTransport::External {
        let keys = declared_secret_keys(request)?;
        let outcome = provider
            .push_secrets(SecretBatch::Declared {
                purpose: SecretPurpose::Profile,
                keys,
            })
            .await?;
        let required = match outcome {
            SecretsOutcome::External { required } | SecretsOutcome::Pushed { keys: required } => {
                required
            },
        };
        progress.event(&DeployEvent::SecretsExternal { keys: &required });
        return Ok(());
    }

    match &request.secrets {
        DeploySecretsSource::EnvFromFile { path } => {
            push_file_secrets(provider, request, path, progress).await?;
        },
        DeploySecretsSource::Vault { bootstrap_env } => {
            push_vault_bootstrap(provider, bootstrap_env, progress).await?;
        },
    }

    if let Some(credentials) = request.platform_credentials.as_ref() {
        progress.event(&DeployEvent::CredentialsSyncStarted);
        let count = push(
            provider,
            SecretPurpose::PlatformCredentials,
            credentials_env(credentials),
        )
        .await?;
        progress.event(&DeployEvent::CredentialsSynced { count });
    }

    let mut profile_secret = HashMap::new();
    profile_secret.insert(PROFILE_ENV.to_owned(), profile_env_path(request));
    push(provider, SecretPurpose::ProfilePath, profile_secret).await?;
    progress.event(&DeployEvent::ProfilePathConfigured);

    Ok(())
}

pub(super) fn profile_env_path(request: &DeployRequest) -> String {
    format!(
        "{}/{}/{}",
        container::PROFILES,
        request.profile_name,
        paths::PROFILE_CONFIG
    )
}

pub(super) fn declared_secret_keys(request: &DeployRequest) -> Result<Vec<String>> {
    let mut keys: Vec<String> = match &request.secrets {
        DeploySecretsSource::EnvFromFile { path } => {
            let mut keys: Vec<String> = if path.exists() {
                secrets_env::map_secrets_to_env_vars(secrets_env::load_secrets_json(path)?)
                    .into_keys()
                    .collect()
            } else {
                Vec::new()
            };
            if !keys.iter().any(|key| key == SIGNING_KEY_ENV)
                && secrets_env::read_signing_key_pem(&request.signing_key_path)?.is_some()
            {
                keys.push(SIGNING_KEY_ENV.to_owned());
            }
            keys
        },
        DeploySecretsSource::Vault { bootstrap_env } => bootstrap_env.clone(),
    };
    keys.push(PROFILE_ENV.to_owned());
    keys.sort();
    keys.dedup();
    Ok(keys)
}

pub(super) async fn push_file_secrets<P: DeployProvider>(
    provider: &P,
    request: &DeployRequest,
    path: &Path,
    progress: &dyn DeployProgress,
) -> Result<()> {
    let mut env_secrets = if path.exists() {
        secrets_env::map_secrets_to_env_vars(secrets_env::load_secrets_json(path)?)
    } else {
        progress.event(&DeployEvent::SecretsFileMissing);
        HashMap::new()
    };

    if !env_secrets.contains_key(SIGNING_KEY_ENV)
        && let Some(pem) = secrets_env::read_signing_key_pem(&request.signing_key_path)?
    {
        env_secrets.insert(SIGNING_KEY_ENV.to_owned(), pem);
    }

    if !env_secrets.is_empty() {
        progress.event(&DeployEvent::SecretsSyncStarted);
        let count = push(provider, SecretPurpose::Profile, env_secrets).await?;
        progress.event(&DeployEvent::SecretsSynced { count });
    }
    Ok(())
}

pub(super) async fn push_vault_bootstrap<P: DeployProvider>(
    provider: &P,
    bootstrap_env: &[String],
    progress: &dyn DeployProgress,
) -> Result<()> {
    let bootstrap = collect_bootstrap_env(bootstrap_env, |name| std::env::var(name).ok())?;
    if bootstrap.is_empty() {
        return Ok(());
    }
    progress.event(&DeployEvent::SecretsSyncStarted);
    let count = push(provider, SecretPurpose::VaultBootstrap, bootstrap).await?;
    progress.event(&DeployEvent::VaultBootstrapSynced { count });
    Ok(())
}

pub fn collect_bootstrap_env(
    names: &[String],
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<HashMap<String, String>> {
    let mut collected = HashMap::new();
    let mut missing = Vec::new();
    for name in names {
        match lookup(name).filter(|value| !value.trim().is_empty()) {
            Some(value) => {
                collected.insert(name.clone(), value);
            },
            None => missing.push(name.clone()),
        }
    }
    if !missing.is_empty() {
        bail!(
            "The profile fetches its secrets from Vault, so the deploy pushes only the Vault \
             bootstrap credentials. These are not set in this shell: {}.",
            missing.join(", ")
        );
    }
    Ok(collected)
}

pub(super) fn credentials_env(creds: &CloudCredentials) -> HashMap<String, String> {
    HashMap::from([
        (
            "SYSTEMPROMPT_API_TOKEN".to_owned(),
            creds.api_token.as_str().to_owned(),
        ),
        (
            "SYSTEMPROMPT_USER_EMAIL".to_owned(),
            creds.user_email.as_str().to_owned(),
        ),
        ("SYSTEMPROMPT_CLI_REMOTE".to_owned(), "true".to_owned()),
    ])
}

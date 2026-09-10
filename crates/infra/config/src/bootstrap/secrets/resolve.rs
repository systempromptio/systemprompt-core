//! Pure precedence resolution for the secrets source.
//!
//! The decision is separated from every side effect so the ordering can be
//! exercised directly: nothing here reads the environment, the filesystem, or
//! the network. Callers pass the already-observed facts.
//!
//! Precedence, highest first:
//! 1. subprocess marker — the parent handed the child its secrets as env
//! 2. `source: vault` — wins even on deployment hosts, so a container is not
//!    silently downgraded to whatever env happens to be set
//! 3. deployment host — env, with or without a pepper already present
//! 4. `source: env` locally — file first, env as fallback
//! 5. `source: file`
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::profile::{SecretsConfig, SecretsSource, VaultSecretsConfig};

use super::SecretsBootstrapError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSource<'a> {
    SubprocessEnv,
    Vault(&'a VaultSecretsConfig),
    DeploymentHostEnv,
    LocalEnvWithFileFallback(&'a str),
    File(&'a str),
}

pub fn resolve_source(
    config: Option<&SecretsConfig>,
    is_subprocess: bool,
    is_deployment_host: bool,
    has_valid_pepper_in_env: bool,
) -> Result<ResolvedSource<'_>, SecretsBootstrapError> {
    if is_subprocess && has_valid_pepper_in_env {
        return Ok(ResolvedSource::SubprocessEnv);
    }

    let Some(config) = config else {
        return if is_deployment_host && has_valid_pepper_in_env {
            Ok(ResolvedSource::DeploymentHostEnv)
        } else {
            Err(SecretsBootstrapError::NoSecretsConfigured)
        };
    };

    if matches!(config.source, SecretsSource::Vault) {
        return config
            .vault
            .as_ref()
            .map(ResolvedSource::Vault)
            .ok_or(SecretsBootstrapError::VaultBlockMissing);
    }

    if is_deployment_host
        && (has_valid_pepper_in_env || matches!(config.source, SecretsSource::Env))
    {
        return Ok(ResolvedSource::DeploymentHostEnv);
    }

    match config.source {
        SecretsSource::Env => Ok(ResolvedSource::LocalEnvWithFileFallback(configured_path(
            config,
        )?)),
        SecretsSource::File | SecretsSource::Vault => {
            Ok(ResolvedSource::File(configured_path(config)?))
        },
    }
}

fn configured_path(config: &SecretsConfig) -> Result<&str, SecretsBootstrapError> {
    config
        .secrets_path()
        .map_err(|e| SecretsBootstrapError::SecretsConfigInvalid {
            message: e.to_string(),
        })
}

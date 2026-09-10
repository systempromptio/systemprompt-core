//! Dispatcher from the resolved source to the loader that serves it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::profile::{SecretsValidationMode, VaultSecretsConfig};
use systemprompt_models::secrets::{OAUTH_AT_REST_PEPPER_MIN_LENGTH, Secrets};

use super::io::handle_load_error;
use super::provider::SecretsProvider;
use super::resolve::{ResolvedSource, resolve_source};
use super::vault::VaultKvProvider;
use super::{SecretsBootstrapError, log_secrets_issue};
use crate::bootstrap::profile::ProfileBootstrap;
use crate::bootstrap::secrets::sources::{env, file};
use crate::error::{ConfigError, ConfigResult};

pub(super) async fn load_from_profile_config() -> ConfigResult<Secrets> {
    let is_deployment_host =
        systemprompt_models::subprocess::is_deployment_host(|name| std::env::var(name).ok());
    let is_subprocess = std::env::var("SYSTEMPROMPT_SUBPROCESS").is_ok();
    let has_valid_pepper_in_env = std::env::var("OAUTH_AT_REST_PEPPER")
        .is_ok_and(|pepper| pepper.len() >= OAUTH_AT_REST_PEPPER_MIN_LENGTH);

    let secrets_config = match ProfileBootstrap::get() {
        Ok(profile) => profile.secrets.as_ref(),
        Err(_e) if (is_subprocess || is_deployment_host) && has_valid_pepper_in_env => None,
        Err(_e) => return Err(SecretsBootstrapError::ProfileNotInitialized.into()),
    };
    let validation = secrets_config.map_or_else(SecretsValidationMode::default, |c| c.validation);

    let resolved = resolve_source(
        secrets_config,
        is_subprocess,
        is_deployment_host,
        has_valid_pepper_in_env,
    )?;

    match resolved {
        ResolvedSource::SubprocessEnv | ResolvedSource::DeploymentHostEnv => {
            tracing::debug!("Loading secrets from environment");
            env::load_from_env()
        },
        ResolvedSource::LocalEnvWithFileFallback(path) => {
            tracing::debug!("Profile source is 'env' but running locally, trying file first");
            file::resolve_and_load_file(path).or_else(|_e| {
                tracing::debug!("File load failed, falling back to environment");
                env::load_from_env()
            })
        },
        ResolvedSource::File(path) => {
            tracing::debug!("Loading secrets from file (profile source: file)");
            file::resolve_and_load_file(path).or_else(|e| handle_load_error(e, validation))
        },
        ResolvedSource::Vault(cfg) => load_from_vault(cfg, validation).await,
    }
}

async fn load_from_vault(
    cfg: &VaultSecretsConfig,
    validation: SecretsValidationMode,
) -> ConfigResult<Secrets> {
    // Why: a Vault failure is never rescued by the environment — a downgraded
    // boot would run on whatever stale credentials the host happens to carry.
    let provider = match VaultKvProvider::from_config(cfg, |name| std::env::var(name).ok()) {
        Ok(provider) => provider,
        Err(e) => return Err(fail_closed(SecretsBootstrapError::from(e), validation)),
    };

    tracing::debug!(source = %provider.describe(), "loading secrets from vault");

    let document = match provider.fetch().await {
        Ok(document) => document,
        Err(e) => return Err(fail_closed(e, validation)),
    };
    let key_names = document.key_names();

    match document.into_secrets() {
        Ok(secrets) => {
            tracing::debug!(keys = ?key_names, "vault secrets document parsed");
            Ok(secrets)
        },
        Err(e) => Err(fail_closed(e, validation)),
    }
}

fn fail_closed(e: SecretsBootstrapError, validation: SecretsValidationMode) -> ConfigError {
    let error = ConfigError::from(e);
    log_secrets_issue(&error, validation);
    error
}

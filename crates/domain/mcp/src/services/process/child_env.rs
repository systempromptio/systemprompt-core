//! The secret half of a spawned MCP server's environment: the granted
//! secrets and the deployment's public verification key set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_config::SecretsService;
use systemprompt_manifest::Config;
use systemprompt_manifest::child_env::ChildBoot;
use systemprompt_models::subprocess::{SecretGrant, VERIFICATION_KEYS_ENV};
use systemprompt_security::keys::SigningKeys;

use crate::McpServerConfig;
use crate::error::McpDomainResult;

fn verification_keys_json(
    global: &Config,
    secrets: &SecretsService,
    config: &McpServerConfig,
) -> McpDomainResult<String> {
    let keys = SigningKeys::resolve(&global.signing_key_path, secrets).map_err(|e| {
        crate::error::McpDomainError::operation(
            format!(
                "Failed to resolve the verification keys for {}",
                config.name
            ),
            e,
        )
    })?;
    serde_json::to_string(keys.verification_keys()).map_err(|e| {
        crate::error::McpDomainError::operation(
            format!(
                "Failed to serialise the verification keys for {}",
                config.name
            ),
            e,
        )
    })
}

pub(super) fn child_secrets(
    global: &Config,
    secrets: &SecretsService,
    config: &McpServerConfig,
) -> McpDomainResult<Vec<(String, String)>> {
    let snapshot = secrets.get();
    if let Some(name) = config
        .env_vars
        .iter()
        .find(|name| snapshot.is_secret_env_name(name))
    {
        return Err(crate::error::McpDomainError::Configuration(format!(
            "MCP server '{}': env_vars names '{name}', which carries a secret; grant it under \
             `secrets:` instead",
            config.name
        )));
    }
    let mut env = secrets
        .child_env(ChildBoot::Host, &config.secrets, &[])
        .map_err(|e| {
            crate::error::McpDomainError::operation(
                format!("Failed to resolve the granted secrets for {}", config.name),
                e,
            )
        })?;
    env.push((
        VERIFICATION_KEYS_ENV.to_owned(),
        verification_keys_json(global, secrets, config)?,
    ));
    if config.secrets.contains(&SecretGrant::EncryptionMasterKey) {
        tracing::info!(server = %config.name, grant = "encryption_master_key", "MCP server granted the encryption master key");
    }
    Ok(env)
}

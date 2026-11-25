//! The secret half of a spawned MCP server's environment: the granted
//! secrets and the deployment's public verification key set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_config::ChildSecretEnv;
use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;
use systemprompt_security::keys::TokenVerificationKeys;

use crate::McpServerConfig;
use crate::error::McpDomainResult;

fn verification_keys_json(
    keys: &TokenVerificationKeys,
    config: &McpServerConfig,
) -> McpDomainResult<String> {
    serde_json::to_string(keys).map_err(|e| {
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
    secrets: &dyn ChildSecretEnv,
    keys: &TokenVerificationKeys,
    config: &McpServerConfig,
) -> McpDomainResult<Vec<(String, String)>> {
    secrets.refused_env_var(&config.env_vars).map_err(|e| {
        crate::error::McpDomainError::invalid_configuration(
            format!("MCP server '{}'", config.name),
            e,
        )
    })?;
    let mut env = secrets.child_env(&config.secrets, &[]).map_err(|e| {
        crate::error::McpDomainError::operation(
            format!("Failed to resolve the granted secrets for {}", config.name),
            e,
        )
    })?;
    env.push((
        VERIFICATION_KEYS_ENV.to_owned(),
        verification_keys_json(keys, config)?,
    ));
    Ok(env)
}

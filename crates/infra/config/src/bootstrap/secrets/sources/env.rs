//! Secrets document assembled from the sanctioned environment variables.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_models::paths::constants::env_vars;
use systemprompt_models::read_env_optional;
use systemprompt_models::secrets::Secrets;

use crate::bootstrap::secrets::SecretsBootstrapError;
use crate::error::ConfigResult;

pub(in crate::bootstrap::secrets) fn load_from_env() -> ConfigResult<Secrets> {
    let oauth_at_rest_pepper = read_env_required(
        "OAUTH_AT_REST_PEPPER",
        SecretsBootstrapError::OauthAtRestPepperRequired,
    )?;
    let database_url =
        read_env_required("DATABASE_URL", SecretsBootstrapError::DatabaseUrlRequired)?;

    let custom = read_env_optional(env_vars::CUSTOM_SECRETS).map_or_else(HashMap::new, |keys| {
        keys.split(',')
            .filter_map(|key| {
                let key = key.trim();
                read_env_optional(key).map(|v| (key.to_owned(), v))
            })
            .collect()
    });

    let secrets = Secrets {
        oauth_at_rest_pepper,
        manifest_signing_secret_seed: read_env_optional("MANIFEST_SIGNING_SECRET_SEED"),
        signing_key_pem: read_env_optional("SIGNING_KEY_PEM"),
        database_url,
        database_write_url: read_env_optional("DATABASE_WRITE_URL"),
        external_database_url: read_env_optional("EXTERNAL_DATABASE_URL"),
        internal_database_url: read_env_optional("INTERNAL_DATABASE_URL"),
        gemini: read_env_optional("GEMINI_API_KEY"),
        anthropic: read_env_optional("ANTHROPIC_API_KEY"),
        openai: read_env_optional("OPENAI_API_KEY"),
        github: read_env_optional("GITHUB_TOKEN"),
        moonshot: read_env_optional("MOONSHOT_API_KEY")
            .or_else(|| read_env_optional("KIMI_API_KEY")),
        qwen: read_env_optional("QWEN_API_KEY").or_else(|| read_env_optional("DASHSCOPE_API_KEY")),
        custom,
    };

    secrets.validate()?;
    Ok(secrets)
}

fn read_env_required(name: &str, missing: SecretsBootstrapError) -> ConfigResult<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Ok(v),
        Ok(_) | Err(_) => Err(missing.into()),
    }
}

//! File and environment loaders for the secrets singleton.
//!
//! Resolves the secrets document from the active profile, falling back
//! to environment variables in subprocess and Fly.io container modes.

use std::collections::HashMap;
use std::path::Path;

use systemprompt_models::paths::constants::env_vars;
use systemprompt_models::profile::{SecretsSource, resolve_with_home};
use systemprompt_models::secrets::Secrets;

use super::{JWT_SECRET_MIN_LENGTH, SecretsBootstrapError, handle_load_error};
use crate::bootstrap::profile::ProfileBootstrap;
use crate::error::{ConfigError, ConfigResult};

pub(super) fn load_from_profile_config() -> ConfigResult<Secrets> {
    let is_fly_environment = std::env::var("FLY_APP_NAME").is_ok();
    let is_subprocess = std::env::var("SYSTEMPROMPT_SUBPROCESS").is_ok();

    if is_subprocess || is_fly_environment {
        if let Ok(jwt_secret) = std::env::var("JWT_SECRET") {
            if jwt_secret.len() >= JWT_SECRET_MIN_LENGTH {
                tracing::debug!("Using JWT_SECRET from environment (subprocess/container mode)");
                return load_from_env();
            }
        }
    }

    let profile =
        ProfileBootstrap::get().map_err(|_| SecretsBootstrapError::ProfileNotInitialized)?;

    let secrets_config = profile
        .secrets
        .as_ref()
        .ok_or(SecretsBootstrapError::NoSecretsConfigured)?;

    let is_fly_environment = std::env::var("FLY_APP_NAME").is_ok();

    match secrets_config.source {
        SecretsSource::Env if is_fly_environment => {
            tracing::debug!("Loading secrets from environment (Fly.io container)");
            load_from_env()
        },
        SecretsSource::Env => {
            tracing::debug!("Profile source is 'env' but running locally, trying file first...");
            resolve_and_load_file(&secrets_config.secrets_path).or_else(|_| {
                tracing::debug!("File load failed, falling back to environment");
                load_from_env()
            })
        },
        SecretsSource::File => {
            tracing::debug!("Loading secrets from file (profile source: file)");
            resolve_and_load_file(&secrets_config.secrets_path)
                .or_else(|e| handle_load_error(e, secrets_config.validation))
        },
    }
}

fn resolve_and_load_file(path_str: &str) -> ConfigResult<Secrets> {
    let profile_path =
        ProfileBootstrap::get_path().map_err(|_| SecretsBootstrapError::ProfileNotInitialized)?;

    let profile_dir = Path::new(profile_path)
        .parent()
        .ok_or_else(|| ConfigError::other("Invalid profile path - no parent directory"))?;

    let resolved_path = resolve_with_home(profile_dir, path_str);
    load_from_file(&resolved_path)
}

fn load_from_file(path: &Path) -> ConfigResult<Secrets> {
    if !path.exists() {
        return Err(SecretsBootstrapError::FileNotFound {
            path: path.display().to_string(),
        }
        .into());
    }

    let content = std::fs::read_to_string(path)?;

    let secrets =
        Secrets::parse(&content).map_err(|e| SecretsBootstrapError::InvalidSecretsFile {
            message: e.to_string(),
        })?;

    tracing::debug!("Loaded secrets from {}", path.display());

    Ok(secrets)
}

fn load_from_env() -> ConfigResult<Secrets> {
    let jwt_secret = read_env_required("JWT_SECRET", SecretsBootstrapError::JwtSecretRequired)?;
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
        jwt_secret,
        manifest_signing_secret_seed: read_env_optional("MANIFEST_SIGNING_SECRET_SEED"),
        database_url,
        database_write_url: read_env_optional("DATABASE_WRITE_URL"),
        external_database_url: read_env_optional("EXTERNAL_DATABASE_URL"),
        internal_database_url: read_env_optional("INTERNAL_DATABASE_URL"),
        sync_token: read_env_optional("SYNC_TOKEN"),
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

fn read_env_optional(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        Ok(_) | Err(_) => None,
    }
}

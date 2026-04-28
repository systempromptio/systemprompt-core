use anyhow::{Context, Result};
use base64::Engine;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::manifest_seed::{MANIFEST_SIGNING_SEED_BYTES, decode_seed, generate_seed, persist_seed};
use crate::paths::constants::env_vars;
use crate::profile::{SecretsSource, SecretsValidationMode, resolve_with_home};
use crate::profile_bootstrap::ProfileBootstrap;
use crate::secrets::{JWT_SECRET_MIN_LENGTH, SECRETS, Secrets};

#[derive(Debug, Clone, Copy)]
pub struct SecretsBootstrap;

#[derive(Debug, thiserror::Error)]
pub enum SecretsBootstrapError {
    #[error(
        "Secrets not initialized. Call SecretsBootstrap::init() after ProfileBootstrap::init()"
    )]
    NotInitialized,

    #[error("Secrets already initialized")]
    AlreadyInitialized,

    #[error("Profile not initialized. Call ProfileBootstrap::init() first")]
    ProfileNotInitialized,

    #[error("Secrets file not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid secrets file: {message}")]
    InvalidSecretsFile { message: String },

    #[error("No secrets configured. Create a secrets.json file.")]
    NoSecretsConfigured,

    #[error(
        "JWT secret is required. Add 'jwt_secret' to your secrets file or set JWT_SECRET \
         environment variable."
    )]
    JwtSecretRequired,

    #[error(
        "Database URL is required. Add 'database_url' to your secrets.json or set DATABASE_URL \
         environment variable."
    )]
    DatabaseUrlRequired,

    #[error(
        "manifest_signing_secret_seed is missing from the secrets file and the bootstrap path is \
         not writable. Run `systemprompt admin cowork rotate-signing-key` against a writable \
         secrets file, or add a base64-encoded 32-byte value under `manifest_signing_secret_seed`."
    )]
    ManifestSeedUnavailable,

    #[error("manifest_signing_secret_seed is invalid: {message}")]
    ManifestSeedInvalid { message: String },
}

impl SecretsBootstrap {
    pub fn init() -> Result<&'static Secrets> {
        if SECRETS.get().is_some() {
            anyhow::bail!(SecretsBootstrapError::AlreadyInitialized);
        }

        let mut secrets = Self::load_from_profile_config()?;
        Self::ensure_manifest_signing_seed(&mut secrets)?;

        Self::log_loaded_secrets(&secrets);

        SECRETS
            .set(secrets)
            .map_err(|_| anyhow::anyhow!(SecretsBootstrapError::AlreadyInitialized))?;

        SECRETS
            .get()
            .ok_or_else(|| anyhow::anyhow!(SecretsBootstrapError::NotInitialized))
    }

    pub fn jwt_secret() -> Result<&'static str, SecretsBootstrapError> {
        Ok(&Self::get()?.jwt_secret)
    }

    pub fn manifest_signing_secret_seed()
    -> Result<[u8; MANIFEST_SIGNING_SEED_BYTES], SecretsBootstrapError> {
        let encoded = Self::get()?
            .manifest_signing_secret_seed
            .as_deref()
            .ok_or(SecretsBootstrapError::ManifestSeedUnavailable)?;
        decode_seed(encoded)
    }

    pub fn rotate_manifest_signing_seed() -> Result<[u8; MANIFEST_SIGNING_SEED_BYTES]> {
        let path = Self::resolved_secrets_file_path()
            .context("rotate-signing-key requires a file-backed secrets source")?;
        let seed = generate_seed();
        persist_seed(&path, &seed)?;
        Ok(seed)
    }

    fn ensure_manifest_signing_seed(secrets: &mut Secrets) -> Result<()> {
        if let Some(encoded) = secrets.manifest_signing_secret_seed.as_deref() {
            decode_seed(encoded)?;
            return Ok(());
        }
        if std::env::var("SYSTEMPROMPT_SUBPROCESS").is_ok() {
            anyhow::bail!(
                "manifest_signing_secret_seed missing in subprocess env — parent must propagate \
                 MANIFEST_SIGNING_SECRET_SEED so subprocesses don't regenerate and clobber the \
                 secrets file"
            );
        }
        let Ok(path) = Self::resolved_secrets_file_path() else {
            tracing::warn!(
                "manifest_signing_secret_seed missing and no writable secrets file is configured"
            );
            return Ok(());
        };
        if !path.exists() {
            tracing::warn!(
                path = %path.display(),
                "manifest_signing_secret_seed missing and secrets file does not exist on disk"
            );
            return Ok(());
        }
        let seed = generate_seed();
        persist_seed(&path, &seed)?;
        secrets.manifest_signing_secret_seed =
            Some(base64::engine::general_purpose::STANDARD.encode(seed));
        tracing::info!(
            path = %path.display(),
            "Generated and persisted fresh manifest_signing_secret_seed"
        );
        Ok(())
    }

    fn resolved_secrets_file_path() -> Result<PathBuf> {
        let profile =
            ProfileBootstrap::get().map_err(|_| SecretsBootstrapError::ProfileNotInitialized)?;
        let secrets_config = profile
            .secrets
            .as_ref()
            .ok_or(SecretsBootstrapError::NoSecretsConfigured)?;
        let profile_path = ProfileBootstrap::get_path()
            .context("SYSTEMPROMPT_PROFILE not set - cannot resolve secrets path")?;
        let profile_dir = Path::new(profile_path)
            .parent()
            .context("Invalid profile path - no parent directory")?;
        Ok(resolve_with_home(profile_dir, &secrets_config.secrets_path))
    }

    pub fn database_url() -> Result<&'static str, SecretsBootstrapError> {
        Ok(&Self::get()?.database_url)
    }

    pub fn database_write_url() -> Result<Option<&'static str>, SecretsBootstrapError> {
        Ok(Self::get()?.database_write_url.as_deref())
    }

    fn load_from_env() -> Result<Secrets> {
        let jwt_secret = std::env::var("JWT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or(SecretsBootstrapError::JwtSecretRequired)?;

        let database_url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or(SecretsBootstrapError::DatabaseUrlRequired)?;

        let custom = std::env::var(env_vars::CUSTOM_SECRETS)
            .ok()
            .filter(|s| !s.is_empty())
            .map_or_else(HashMap::new, |keys| {
                keys.split(',')
                    .filter_map(|key| {
                        let key = key.trim();
                        std::env::var(key)
                            .ok()
                            .filter(|v| !v.is_empty())
                            .map(|v| (key.to_owned(), v))
                    })
                    .collect()
            });

        let secrets = Secrets {
            jwt_secret,
            manifest_signing_secret_seed: std::env::var("MANIFEST_SIGNING_SECRET_SEED")
                .ok()
                .filter(|s| !s.is_empty()),
            database_url,
            database_write_url: std::env::var("DATABASE_WRITE_URL")
                .ok()
                .filter(|s| !s.is_empty()),
            external_database_url: std::env::var("EXTERNAL_DATABASE_URL")
                .ok()
                .filter(|s| !s.is_empty()),
            internal_database_url: std::env::var("INTERNAL_DATABASE_URL")
                .ok()
                .filter(|s| !s.is_empty()),
            sync_token: std::env::var("SYNC_TOKEN").ok().filter(|s| !s.is_empty()),
            gemini: std::env::var("GEMINI_API_KEY")
                .ok()
                .filter(|s| !s.is_empty()),
            anthropic: std::env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|s| !s.is_empty()),
            openai: std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|s| !s.is_empty()),
            github: std::env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty()),
            moonshot: std::env::var("MOONSHOT_API_KEY")
                .ok()
                .or_else(|| std::env::var("KIMI_API_KEY").ok())
                .filter(|s| !s.is_empty()),
            qwen: std::env::var("QWEN_API_KEY")
                .ok()
                .or_else(|| std::env::var("DASHSCOPE_API_KEY").ok())
                .filter(|s| !s.is_empty()),
            custom,
        };

        secrets.validate()?;
        Ok(secrets)
    }

    fn load_from_profile_config() -> Result<Secrets> {
        let is_fly_environment = std::env::var("FLY_APP_NAME").is_ok();
        let is_subprocess = std::env::var("SYSTEMPROMPT_SUBPROCESS").is_ok();

        if is_subprocess || is_fly_environment {
            if let Ok(jwt_secret) = std::env::var("JWT_SECRET") {
                if jwt_secret.len() >= JWT_SECRET_MIN_LENGTH {
                    tracing::debug!(
                        "Using JWT_SECRET from environment (subprocess/container mode)"
                    );
                    return Self::load_from_env();
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
                Self::load_from_env()
            },
            SecretsSource::Env => {
                tracing::debug!(
                    "Profile source is 'env' but running locally, trying file first..."
                );
                Self::resolve_and_load_file(&secrets_config.secrets_path).or_else(|_| {
                    tracing::debug!("File load failed, falling back to environment");
                    Self::load_from_env()
                })
            },
            SecretsSource::File => {
                tracing::debug!("Loading secrets from file (profile source: file)");
                Self::resolve_and_load_file(&secrets_config.secrets_path)
                    .or_else(|e| Self::handle_load_error(e, secrets_config.validation))
            },
        }
    }

    fn handle_load_error(e: anyhow::Error, mode: SecretsValidationMode) -> Result<Secrets> {
        log_secrets_issue(&e, mode);
        Err(e)
    }

    pub fn get() -> Result<&'static Secrets, SecretsBootstrapError> {
        SECRETS.get().ok_or(SecretsBootstrapError::NotInitialized)
    }

    pub fn require() -> Result<&'static Secrets, SecretsBootstrapError> {
        Self::get()
    }

    pub fn is_initialized() -> bool {
        SECRETS.get().is_some()
    }

    pub fn try_init() -> Result<&'static Secrets> {
        if SECRETS.get().is_some() {
            return Self::get().map_err(Into::into);
        }
        Self::init()
    }

    fn resolve_and_load_file(path_str: &str) -> Result<Secrets> {
        let profile_path = ProfileBootstrap::get_path()
            .context("SYSTEMPROMPT_PROFILE not set - cannot resolve secrets path")?;

        let profile_dir = Path::new(profile_path)
            .parent()
            .context("Invalid profile path - no parent directory")?;

        let resolved_path = resolve_with_home(profile_dir, path_str);
        Self::load_from_file(&resolved_path)
    }

    fn load_from_file(path: &Path) -> Result<Secrets> {
        if !path.exists() {
            anyhow::bail!(SecretsBootstrapError::FileNotFound {
                path: path.display().to_string()
            });
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read secrets file: {}", path.display()))?;

        let secrets = Secrets::parse(&content).map_err(|e| {
            anyhow::anyhow!(SecretsBootstrapError::InvalidSecretsFile {
                message: e.to_string(),
            })
        })?;

        tracing::debug!("Loaded secrets from {}", path.display());

        Ok(secrets)
    }

    fn log_loaded_secrets(secrets: &Secrets) {
        let message = build_loaded_secrets_message(secrets);
        tracing::debug!("{}", message);
    }
}

pub fn log_secrets_issue(e: &anyhow::Error, mode: SecretsValidationMode) {
    match mode {
        SecretsValidationMode::Warn => log_secrets_warn(e),
        SecretsValidationMode::Skip => log_secrets_skip(e),
        SecretsValidationMode::Strict => {},
    }
}

pub fn log_secrets_warn(e: &anyhow::Error) {
    tracing::warn!("Secrets file issue: {}", e);
}

pub fn log_secrets_skip(e: &anyhow::Error) {
    tracing::debug!("Skipping secrets file: {}", e);
}

pub fn build_loaded_secrets_message(secrets: &Secrets) -> String {
    let base = ["jwt_secret", "database_url"];
    let optional_providers = [
        secrets
            .database_write_url
            .as_ref()
            .map(|_| "database_write_url"),
        secrets
            .external_database_url
            .as_ref()
            .map(|_| "external_database_url"),
        secrets
            .internal_database_url
            .as_ref()
            .map(|_| "internal_database_url"),
        secrets.gemini.as_ref().map(|_| "gemini"),
        secrets.anthropic.as_ref().map(|_| "anthropic"),
        secrets.openai.as_ref().map(|_| "openai"),
        secrets.github.as_ref().map(|_| "github"),
    ];

    let loaded: Vec<&str> = base
        .into_iter()
        .chain(optional_providers.into_iter().flatten())
        .collect();

    if secrets.custom.is_empty() {
        format!("Loaded secrets: {}", loaded.join(", "))
    } else {
        format!(
            "Loaded secrets: {}, {} custom",
            loaded.join(", "),
            secrets.custom.len()
        )
    }
}

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use crate::profile::{resolve_with_home, SecretsSource, SecretsValidationMode};
use crate::profile_bootstrap::ProfileBootstrap;

static SECRETS: OnceLock<Secrets> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secrets {
    pub jwt_secret: String,

    pub database_url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,

    #[serde(default, flatten)]
    pub custom: HashMap<String, String>,
}

const JWT_SECRET_MIN_LENGTH: usize = 32;

impl Secrets {
    pub fn parse(content: &str) -> Result<Self> {
        let secrets: Self =
            serde_json::from_str(content).context("Failed to parse secrets JSON")?;
        secrets.validate()?;
        Ok(secrets)
    }

    fn validate(&self) -> Result<()> {
        if self.jwt_secret.len() < JWT_SECRET_MIN_LENGTH {
            anyhow::bail!(
                "jwt_secret must be at least {} characters (got {})",
                JWT_SECRET_MIN_LENGTH,
                self.jwt_secret.len()
            );
        }
        Ok(())
    }

    pub const fn has_ai_provider(&self) -> bool {
        self.gemini.is_some() || self.anthropic.is_some() || self.openai.is_some()
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        match key {
            "jwt_secret" | "JWT_SECRET" => Some(&self.jwt_secret),
            "database_url" | "DATABASE_URL" => Some(&self.database_url),
            "gemini" | "GEMINI_API_KEY" => self.gemini.as_ref(),
            "anthropic" | "ANTHROPIC_API_KEY" => self.anthropic.as_ref(),
            "openai" | "OPENAI_API_KEY" => self.openai.as_ref(),
            "github" | "GITHUB_TOKEN" => self.github.as_ref(),
            other => self.custom.get(other),
        }
    }

    pub fn log_configured_providers(&self) {
        let configured: Vec<&str> = [
            self.gemini.as_ref().map(|_| "gemini"),
            self.anthropic.as_ref().map(|_| "anthropic"),
            self.openai.as_ref().map(|_| "openai"),
            self.github.as_ref().map(|_| "github"),
        ]
        .into_iter()
        .flatten()
        .collect();

        tracing::info!(providers = ?configured, "Configured API providers");
    }
}

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
}

impl SecretsBootstrap {
    pub fn init() -> Result<&'static Secrets> {
        if SECRETS.get().is_some() {
            anyhow::bail!(SecretsBootstrapError::AlreadyInitialized);
        }

        let secrets = Self::load_from_profile_config()?;

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

    pub fn database_url() -> Result<&'static str, SecretsBootstrapError> {
        Ok(&Self::get()?.database_url)
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

        let custom = std::env::var("SYSTEMPROMPT_CUSTOM_SECRETS")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|keys| {
                keys.split(',')
                    .filter_map(|key| {
                        let key = key.trim();
                        std::env::var(key)
                            .ok()
                            .filter(|v| !v.is_empty())
                            .map(|v| (key.to_owned(), v))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let secrets = Secrets {
            jwt_secret,
            database_url,
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
                    tracing::debug!("Using JWT_SECRET from environment (subprocess/container mode)");
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

fn log_secrets_issue(e: &anyhow::Error, mode: SecretsValidationMode) {
    match mode {
        SecretsValidationMode::Warn => log_secrets_warn(e),
        SecretsValidationMode::Skip => log_secrets_skip(e),
        SecretsValidationMode::Strict => {},
    }
}

fn log_secrets_warn(e: &anyhow::Error) {
    tracing::warn!("Secrets file issue: {}", e);
}

fn log_secrets_skip(e: &anyhow::Error) {
    tracing::debug!("Skipping secrets file: {}", e);
}

fn build_loaded_secrets_message(secrets: &Secrets) -> String {
    let base = ["jwt_secret", "database_url"];
    let optional_providers = [
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

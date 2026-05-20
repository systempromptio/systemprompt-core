//! Process-wide secrets bootstrap.
//!
//! Loads the secrets document referenced by the active profile (or
//! the equivalent environment variables in subprocess/Fly.io modes),
//! validates required fields, and exposes typed accessors for the
//! manifest signing seed and database URLs.

mod io;
mod loader;
mod logging;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use base64::Engine;
use systemprompt_models::profile::resolve_with_home;
use systemprompt_models::secrets::Secrets;

use super::manifest::{
    MANIFEST_SIGNING_SEED_BYTES, decode_seed, dir_is_writable, generate_seed, persist_seed,
};
use super::profile::ProfileBootstrap;
use crate::error::{ConfigError, ConfigResult};

pub use io::{handle_load_error, load_secrets_from_path};
pub use logging::{
    build_loaded_secrets_message, log_secrets_issue, log_secrets_skip, log_secrets_warn,
};

static SECRETS: OnceLock<Secrets> = OnceLock::new();

pub const JWT_SECRET_MIN_LENGTH: usize = 32;

#[derive(Debug, Clone, Copy)]
pub struct SecretsBootstrap;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
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
        "OAuth at-rest pepper is required. Add 'oauth_at_rest_pepper' (>= 32 chars) to your \
         secrets file or set OAUTH_AT_REST_PEPPER environment variable."
    )]
    OauthAtRestPepperRequired,

    #[error(
        "Database URL is required. Add 'database_url' to your secrets.json or set DATABASE_URL \
         environment variable."
    )]
    DatabaseUrlRequired,

    #[error(
        "manifest_signing_secret_seed is missing from the secrets file and the bootstrap path is \
         not writable. Run `systemprompt admin bridge rotate-signing-key` against a writable \
         secrets file, or add a base64-encoded 32-byte value under `manifest_signing_secret_seed`."
    )]
    ManifestSeedUnavailable,

    #[error("manifest_signing_secret_seed is invalid: {message}")]
    ManifestSeedInvalid { message: String },

    #[error(
        "manifest_signing_secret_seed missing in subprocess env — parent must propagate \
         MANIFEST_SIGNING_SECRET_SEED so subprocesses don't regenerate and clobber the secrets \
         file"
    )]
    SubprocessSeedMissing,
}

impl SecretsBootstrap {
    pub fn init() -> ConfigResult<&'static Secrets> {
        if SECRETS.get().is_some() {
            return Err(SecretsBootstrapError::AlreadyInitialized.into());
        }

        let mut secrets = loader::load_from_profile_config()?;
        Self::ensure_manifest_signing_seed(&mut secrets)?;

        Self::log_loaded_secrets(&secrets);

        SECRETS
            .set(secrets)
            .map_err(|_| SecretsBootstrapError::AlreadyInitialized)?;

        SECRETS
            .get()
            .ok_or_else(|| SecretsBootstrapError::NotInitialized.into())
    }

    pub fn jwt_secret() -> Result<&'static str, SecretsBootstrapError> {
        Ok(&Self::get()?.jwt_secret)
    }

    pub fn oauth_at_rest_pepper() -> Result<&'static str, SecretsBootstrapError> {
        Ok(&Self::get()?.oauth_at_rest_pepper)
    }

    pub fn manifest_signing_secret_seed()
    -> Result<[u8; MANIFEST_SIGNING_SEED_BYTES], SecretsBootstrapError> {
        let encoded = Self::get()?
            .manifest_signing_secret_seed
            .as_deref()
            .ok_or(SecretsBootstrapError::ManifestSeedUnavailable)?;
        decode_seed(encoded)
    }

    pub fn rotate_manifest_signing_seed() -> ConfigResult<[u8; MANIFEST_SIGNING_SEED_BYTES]> {
        let path = Self::resolved_secrets_file_path()?;
        let seed = generate_seed();
        persist_seed(&path, &seed)?;
        Ok(seed)
    }

    fn ensure_manifest_signing_seed(secrets: &mut Secrets) -> ConfigResult<()> {
        if let Some(encoded) = secrets.manifest_signing_secret_seed.as_deref() {
            decode_seed(encoded)?;
            return Ok(());
        }
        if std::env::var("SYSTEMPROMPT_SUBPROCESS").is_ok() {
            return Err(SecretsBootstrapError::SubprocessSeedMissing.into());
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
        secrets.manifest_signing_secret_seed =
            Some(base64::engine::general_purpose::STANDARD.encode(seed));

        // The profile directory may be mounted read-only (e.g. an air-gapped
        // deployment with a `:ro` profile mount). The seed is only needed for
        // manifest signing within this process, so a failed persist is a
        // warning, not a fatal error: the in-memory seed above keeps signing
        // working for this boot. Operators wanting a stable seed across boots
        // should set `MANIFEST_SIGNING_SECRET_SEED` or use a writable dir.
        let profile_dir = path.parent().unwrap_or_else(|| Path::new("."));
        if !dir_is_writable(profile_dir) {
            tracing::warn!(
                path = %path.display(),
                "profile dir is read-only — using an ephemeral manifest signing seed for this \
                 boot; set MANIFEST_SIGNING_SECRET_SEED or use a writable dir to persist it"
            );
            return Ok(());
        }
        if let Err(err) = persist_seed(&path, &seed) {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "could not persist manifest_signing_secret_seed — using an ephemeral seed for \
                 this boot; set MANIFEST_SIGNING_SECRET_SEED to make it stable"
            );
            return Ok(());
        }
        tracing::info!(
            path = %path.display(),
            "Generated and persisted fresh manifest_signing_secret_seed"
        );
        Ok(())
    }

    fn resolved_secrets_file_path() -> ConfigResult<PathBuf> {
        let profile =
            ProfileBootstrap::get().map_err(|_| SecretsBootstrapError::ProfileNotInitialized)?;
        let secrets_config = profile
            .secrets
            .as_ref()
            .ok_or(SecretsBootstrapError::NoSecretsConfigured)?;
        let profile_path = ProfileBootstrap::get_path()
            .map_err(|_| SecretsBootstrapError::ProfileNotInitialized)?;
        let profile_dir = Path::new(profile_path)
            .parent()
            .ok_or_else(|| ConfigError::other("Invalid profile path - no parent directory"))?;
        Ok(resolve_with_home(profile_dir, &secrets_config.secrets_path))
    }

    pub fn database_url() -> Result<&'static str, SecretsBootstrapError> {
        Ok(&Self::get()?.database_url)
    }

    pub fn database_write_url() -> Result<Option<&'static str>, SecretsBootstrapError> {
        Ok(Self::get()?.database_write_url.as_deref())
    }

    pub fn get() -> Result<&'static Secrets, SecretsBootstrapError> {
        SECRETS.get().ok_or(SecretsBootstrapError::NotInitialized)
    }

    pub fn require() -> Result<&'static Secrets, SecretsBootstrapError> {
        Self::get()
    }

    #[must_use]
    pub fn is_initialized() -> bool {
        SECRETS.get().is_some()
    }

    pub fn try_init() -> ConfigResult<&'static Secrets> {
        if SECRETS.get().is_some() {
            return Self::get().map_err(Into::into);
        }
        Self::init()
    }

    fn log_loaded_secrets(secrets: &Secrets) {
        let message = build_loaded_secrets_message(secrets);
        tracing::debug!("{}", message);
    }
}

//! Process-wide secrets bootstrap.
//!
//! Loads the secrets document referenced by the active profile (or
//! the equivalent environment variables in subprocess/Fly.io modes),
//! validates required fields, and exposes typed accessors for the
//! manifest signing seed and database URLs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod io;
mod loader;
mod logging;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use base64::Engine;
use systemprompt_models::profile::resolve_with_home;
use systemprompt_models::secrets::Secrets;

use super::manifest::{MANIFEST_SIGNING_SEED_BYTES, decode_seed, generate_seed, persist_seed};
use super::profile::ProfileBootstrap;
use crate::error::{ConfigError, ConfigResult};

pub use io::load_secrets_from_path;
pub use logging::{
    build_loaded_secrets_message, log_secrets_issue, log_secrets_skip, log_secrets_warn,
};

static SECRETS: OnceLock<Secrets> = OnceLock::new();

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
        "manifest_signing_secret_seed is required: every replica must share one seed, so it is \
         never generated at boot. Run `systemprompt admin identity generate --json` once and \
         distribute the value (secrets file or MANIFEST_SIGNING_SECRET_SEED)."
    )]
    ManifestSeedRequired,

    #[error(
        "signing_key_pem is required on cloud and deployment-host boots: every replica must \
         sign with one key, so it is never read from a file beside the binary there. Run \
         `systemprompt admin identity generate --json` once and distribute the value (secrets \
         file or SIGNING_KEY_PEM)."
    )]
    SigningKeyPemRequired,

    #[error("manifest_signing_secret_seed is invalid: {message}")]
    ManifestSeedInvalid { message: String },

    #[error("signing_key_pem secret is invalid: {message}")]
    SigningKeyPemInvalid { message: String },
}

impl SecretsBootstrap {
    pub fn init() -> ConfigResult<&'static Secrets> {
        if SECRETS.get().is_some() {
            return Err(SecretsBootstrapError::AlreadyInitialized.into());
        }

        let secrets = loader::load_from_profile_config()?;
        Self::validate_identity(&secrets)?;

        Self::log_loaded_secrets(&secrets);

        SECRETS
            .set(secrets)
            .map_err(|_e| SecretsBootstrapError::AlreadyInitialized)?;

        SECRETS
            .get()
            .ok_or_else(|| SecretsBootstrapError::NotInitialized.into())
    }

    pub fn oauth_at_rest_pepper() -> Result<&'static str, SecretsBootstrapError> {
        Ok(&Self::get()?.oauth_at_rest_pepper)
    }

    pub fn signing_key_pem() -> Result<Option<String>, SecretsBootstrapError> {
        let Some(encoded) = Self::get()?.signing_key_pem.as_deref() else {
            return Ok(None);
        };
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| SecretsBootstrapError::SigningKeyPemInvalid {
                message: e.to_string(),
            })?;
        let pem =
            String::from_utf8(bytes).map_err(|e| SecretsBootstrapError::SigningKeyPemInvalid {
                message: e.to_string(),
            })?;
        Ok(Some(pem))
    }

    pub fn manifest_signing_secret_seed()
    -> Result<[u8; MANIFEST_SIGNING_SEED_BYTES], SecretsBootstrapError> {
        let encoded = Self::get()?
            .manifest_signing_secret_seed
            .as_deref()
            .ok_or(SecretsBootstrapError::ManifestSeedRequired)?;
        decode_seed(encoded)
    }

    pub fn rotate_manifest_signing_seed() -> ConfigResult<[u8; MANIFEST_SIGNING_SEED_BYTES]> {
        let path = Self::resolved_secrets_file_path()?;
        let seed = generate_seed();
        persist_seed(&path, &seed)?;
        Ok(seed)
    }

    // Why: identity secrets are inputs. A seed minted at boot gives every
    // replica its own manifest identity, and a key read from a file beside the
    // binary is regenerated per container; both were found by boot failures on
    // a second node. Local profiles may still keep the RSA key at
    // `signing_key_path`, which is why the PEM is only demanded where a file
    // beside the binary cannot be shared.
    fn validate_identity(secrets: &Secrets) -> ConfigResult<()> {
        let encoded = secrets
            .manifest_signing_secret_seed
            .as_deref()
            .ok_or(SecretsBootstrapError::ManifestSeedRequired)?;
        decode_seed(encoded)?;

        let is_deployment_host =
            systemprompt_models::subprocess::is_deployment_host(|name| std::env::var(name).ok());
        let is_cloud = ProfileBootstrap::get().is_ok_and(|profile| profile.target.is_cloud());
        if (is_deployment_host || is_cloud) && secrets.signing_key_pem.is_none() {
            return Err(SecretsBootstrapError::SigningKeyPemRequired.into());
        }
        Ok(())
    }

    fn resolved_secrets_file_path() -> ConfigResult<PathBuf> {
        let profile =
            ProfileBootstrap::get().map_err(|_e| SecretsBootstrapError::ProfileNotInitialized)?;
        let secrets_config = profile
            .secrets
            .as_ref()
            .ok_or(SecretsBootstrapError::NoSecretsConfigured)?;
        let profile_path = ProfileBootstrap::get_path()
            .map_err(|_e| SecretsBootstrapError::ProfileNotInitialized)?;
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
        tracing::debug!("{message}");
    }
}

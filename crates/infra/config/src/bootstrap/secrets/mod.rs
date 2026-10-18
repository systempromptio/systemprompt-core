//! Process-wide secrets bootstrap.
//!
//! Loads the secrets document referenced by the active profile, validates
//! required fields, and exposes typed accessors for the manifest signing seed
//! and database URLs.
//!
//! Source precedence, highest first:
//!
//! | # | Condition | Source |
//! |---|-----------|--------|
//! | 1 | `SYSTEMPROMPT_SUBPROCESS` set and a valid pepper is in the environment | environment |
//! | 2 | `secrets.source: vault` or `external` | Vault KV v2 or a registered provider, on deployment hosts too |
//! | 3 | deployment host, with a valid pepper or `secrets.source: env` | environment |
//! | 4 | `secrets.source: env` running locally | file, then environment |
//! | 5 | `secrets.source: file` | file |
//!
//! Every source is fail-closed: a secrets file or Vault fetch that fails is
//! logged once and aborts the boot instead of falling back to the environment,
//! because a fallback would start the process on whatever stale credentials the
//! host happens to carry. The profile's `secrets.validation` field is not
//! consulted.
//!
//! `secrets.source: external` hands the fetch to the provider an extension
//! registered for the profile's `external.kind` with
//! [`register_secrets_source!`](crate::register_secrets_source). It sits at
//! the same precedence as Vault and is equally fail-closed. The store is
//! reloadable: see [`SecretsBootstrap::reload`].
//!
//! A Vault token is never written into profile YAML — `${VAULT_TOKEN}`
//! interpolation in `profile.yaml` is forbidden. The token comes from the
//! process environment, a file, or an `AppRole` / Kubernetes login.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod io;
mod loader;
mod logging;
mod provider;
mod registry;
mod reload;
mod resolve;
mod sources;
mod vault;

use std::path::{Path, PathBuf};

use base64::Engine;
use systemprompt_manifest::profile::resolve_with_home;
use systemprompt_manifest::secrets::Secrets;

use super::manifest::{MANIFEST_SIGNING_SEED_BYTES, decode_seed, generate_seed, persist_seed};
use super::master_key::decode_master_key;
use super::profile::ProfileBootstrap;
use crate::error::{ConfigError, ConfigResult};

pub use error::SecretsBootstrapError;
pub use io::load_secrets_from_path;
pub use logging::build_loaded_secrets_message;
pub use provider::{DynSecretsProvider, SecretsDocument, SecretsFetch, SecretsProvider};
pub use registry::{
    EnvLookup, SecretsSourceFactory, SecretsSourceRegistration, SharedSecretsProvider,
    build_external_provider, process_env_lookup, registered_external_kinds,
};
pub use reload::{
    PINNED_SECRET_KEYS, ReloadOutcome, SecretsStatus, diff_secrets, refresh_interval_for,
};

pub const BOOT_REQUIRED_SECRET_KEYS: &[&str] = &[
    "database_url",
    "oauth_at_rest_pepper",
    "manifest_signing_secret_seed",
    "encryption_master_key",
];
pub const SIGNING_KEY_PEM: &str = "signing_key_pem";
pub use resolve::{ResolvedSource, resolve_source};
pub use vault::{VaultAttemptFailure, VaultError, VaultKvProvider};

#[derive(Debug, Clone, Copy)]
pub struct SecretsBootstrap;

impl SecretsBootstrap {
    pub async fn init() -> ConfigResult<&'static Secrets> {
        if reload::current().is_some() {
            return Err(SecretsBootstrapError::AlreadyInitialized.into());
        }

        let secrets = loader::load_from_profile_config().await?;
        Self::validate_identity(&secrets)?;

        Self::log_loaded_secrets(&secrets);

        reload::install(secrets).map_err(Into::into)
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
            .map_err(|e| SecretsBootstrapError::SigningKeyPemInvalid(e.into()))?;
        let pem = String::from_utf8(bytes)
            .map_err(|e| SecretsBootstrapError::SigningKeyPemInvalid(e.into()))?;
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

    fn validate_identity(secrets: &Secrets) -> ConfigResult<()> {
        let encoded = secrets
            .manifest_signing_secret_seed
            .as_deref()
            .ok_or(SecretsBootstrapError::ManifestSeedRequired)?;
        decode_seed(encoded)?;

        let master_key = secrets
            .get("encryption_master_key")
            .ok_or(SecretsBootstrapError::EncryptionMasterKeyRequired)?;
        decode_master_key(master_key)?;

        if Self::signing_key_pem_required() && secrets.signing_key_pem.is_none() {
            return Err(SecretsBootstrapError::SigningKeyPemRequired.into());
        }
        Ok(())
    }

    #[must_use]
    pub fn signing_key_pem_required() -> bool {
        let is_deployment_host =
            systemprompt_models::subprocess::is_deployment_host(|name| std::env::var(name).ok());
        is_deployment_host || ProfileBootstrap::get().is_ok_and(|profile| profile.target.is_cloud())
    }

    #[must_use]
    pub fn required_secret_keys(signing_key_pem_required: bool) -> Vec<&'static str> {
        let mut keys = BOOT_REQUIRED_SECRET_KEYS.to_vec();
        if signing_key_pem_required {
            keys.push(SIGNING_KEY_PEM);
        }
        keys
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
        let profile_dir = Path::new(profile_path).parent().ok_or_else(|| {
            ConfigError::ProfilePathWithoutParent {
                path: PathBuf::from(profile_path),
            }
        })?;
        let secrets_path = secrets_config
            .secrets_path()
            .map_err(SecretsBootstrapError::SecretsConfigInvalid)?;
        Ok(resolve_with_home(profile_dir, secrets_path))
    }

    pub fn database_url() -> Result<&'static str, SecretsBootstrapError> {
        Ok(&Self::get()?.database_url)
    }

    pub fn database_write_url() -> Result<Option<&'static str>, SecretsBootstrapError> {
        Ok(Self::get()?.database_write_url.as_deref())
    }

    pub fn get() -> Result<&'static Secrets, SecretsBootstrapError> {
        reload::current().ok_or(SecretsBootstrapError::NotInitialized)
    }

    #[must_use]
    pub fn is_initialized() -> bool {
        reload::current().is_some()
    }

    pub async fn try_init() -> ConfigResult<&'static Secrets> {
        if reload::current().is_some() {
            return Self::get().map_err(Into::into);
        }
        Self::init().await
    }

    fn log_loaded_secrets(secrets: &Secrets) {
        let summary = build_loaded_secrets_message(secrets);
        tracing::debug!(summary = %summary, "Secrets loaded");
    }
}

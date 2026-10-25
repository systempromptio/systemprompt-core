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
//! reloadable: see [`SecretsService::reload`].
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

use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use systemprompt_manifest::profile::{Profile, resolve_with_home};
use systemprompt_manifest::secrets::Secrets;

use super::manifest::{MANIFEST_SIGNING_SEED_BYTES, decode_seed, generate_seed, persist_seed};
use super::master_key::{ENCRYPTION_MASTER_KEY_BYTES, decode_master_key};
use super::profile::LoadedProfile;
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

use reload::SecretsState;

pub const BOOT_REQUIRED_SECRET_KEYS: &[&str] = &[
    "database_url",
    "oauth_at_rest_pepper",
    "manifest_signing_secret_seed",
    "encryption_master_key",
];
pub const SIGNING_KEY_PEM: &str = "signing_key_pem";
pub use resolve::{ResolvedSource, resolve_source};
pub use vault::{VaultAttemptFailure, VaultError, VaultKvProvider};

#[derive(Debug)]
pub struct SecretsService {
    profile: Arc<LoadedProfile>,
    state: SecretsState,
}

impl SecretsService {
    pub async fn load(profile: Arc<LoadedProfile>) -> ConfigResult<Arc<Self>> {
        let secrets = loader::load_from_profile_config(&profile).await?;
        Self::validate_identity(&profile, &secrets)?;

        Self::log_loaded_secrets(&secrets);

        let state = SecretsState::new(secrets, active_source_name(&profile));
        Ok(Arc::new(Self { profile, state }))
    }

    #[must_use]
    pub fn from_secrets(profile: Arc<LoadedProfile>, secrets: Secrets) -> Arc<Self> {
        let state = SecretsState::new(secrets, active_source_name(&profile));
        Arc::new(Self { profile, state })
    }

    #[must_use]
    pub fn get(&self) -> Arc<Secrets> {
        self.state.current()
    }

    #[must_use]
    pub const fn profile(&self) -> &Arc<LoadedProfile> {
        &self.profile
    }

    pub fn oauth_at_rest_pepper(&self) -> String {
        self.get().oauth_at_rest_pepper.clone()
    }

    pub fn encryption_master_key(
        &self,
    ) -> Result<[u8; ENCRYPTION_MASTER_KEY_BYTES], SecretsBootstrapError> {
        let secrets = self.get();
        let encoded = secrets
            .get("encryption_master_key")
            .ok_or(SecretsBootstrapError::EncryptionMasterKeyRequired)?;
        decode_master_key(encoded)
    }

    pub fn signing_key_pem(&self) -> Result<Option<String>, SecretsBootstrapError> {
        let secrets = self.get();
        let Some(encoded) = secrets.signing_key_pem.as_deref() else {
            return Ok(None);
        };
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| SecretsBootstrapError::SigningKeyPemInvalid(e.into()))?;
        let pem = String::from_utf8(bytes)
            .map_err(|e| SecretsBootstrapError::SigningKeyPemInvalid(e.into()))?;
        Ok(Some(pem))
    }

    pub fn manifest_signing_secret_seed(
        &self,
    ) -> Result<[u8; MANIFEST_SIGNING_SEED_BYTES], SecretsBootstrapError> {
        let secrets = self.get();
        let encoded = secrets
            .manifest_signing_secret_seed
            .as_deref()
            .ok_or(SecretsBootstrapError::ManifestSeedRequired)?;
        decode_seed(encoded)
    }

    pub fn rotate_manifest_signing_seed(
        profile: &LoadedProfile,
    ) -> ConfigResult<[u8; MANIFEST_SIGNING_SEED_BYTES]> {
        let path = Self::resolved_secrets_file_path(profile)?;
        let seed = generate_seed();
        persist_seed(&path, &seed)?;
        Ok(seed)
    }

    fn validate_identity(profile: &Profile, secrets: &Secrets) -> ConfigResult<()> {
        let encoded = secrets
            .manifest_signing_secret_seed
            .as_deref()
            .ok_or(SecretsBootstrapError::ManifestSeedRequired)?;
        decode_seed(encoded)?;

        let master_key = secrets
            .get("encryption_master_key")
            .ok_or(SecretsBootstrapError::EncryptionMasterKeyRequired)?;
        decode_master_key(master_key)?;

        if Self::signing_key_pem_required(profile) && secrets.signing_key_pem.is_none() {
            return Err(SecretsBootstrapError::SigningKeyPemRequired.into());
        }
        Ok(())
    }

    #[must_use]
    pub fn signing_key_pem_required(profile: &Profile) -> bool {
        let is_deployment_host =
            systemprompt_models::subprocess::is_deployment_host(|name| std::env::var(name).ok());
        is_deployment_host || profile.target.is_cloud()
    }

    #[must_use]
    pub fn required_secret_keys(signing_key_pem_required: bool) -> Vec<&'static str> {
        let mut keys = BOOT_REQUIRED_SECRET_KEYS.to_vec();
        if signing_key_pem_required {
            keys.push(SIGNING_KEY_PEM);
        }
        keys
    }

    fn resolved_secrets_file_path(profile: &LoadedProfile) -> ConfigResult<PathBuf> {
        let secrets_config = profile
            .secrets
            .as_ref()
            .ok_or(SecretsBootstrapError::NoSecretsConfigured)?;
        let profile_dir = profile.path().parent().ok_or_else(|| {
            ConfigError::ProfilePathWithoutParent {
                path: profile.path().to_path_buf(),
            }
        })?;
        let secrets_path = secrets_config
            .secrets_path()
            .map_err(SecretsBootstrapError::SecretsConfigInvalid)?;
        Ok(resolve_with_home(profile_dir, secrets_path))
    }

    pub fn database_url(&self) -> String {
        self.get().database_url.clone()
    }

    pub fn database_write_url(&self) -> Option<String> {
        self.get().database_write_url.clone()
    }

    fn log_loaded_secrets(secrets: &Secrets) {
        let summary = build_loaded_secrets_message(secrets);
        tracing::debug!(summary = %summary, "Secrets loaded");
    }
}

fn active_source_name(profile: &Profile) -> String {
    profile
        .secrets
        .as_ref()
        .map_or("env", |config| config.source_name())
        .to_owned()
}

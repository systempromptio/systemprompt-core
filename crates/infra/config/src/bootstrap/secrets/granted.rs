//! The secrets a spawned MCP server holds: its grants and nothing else.
//!
//! [`GrantedSecrets::from_lookup`] reads exactly what
//! [`Secrets::child_env`](systemprompt_manifest::secrets::Secrets) emits for
//! a host child: the database URLs, the provider keys and custom secrets its
//! manifest grants (custom names listed in `SYSTEMPROMPT_CUSTOM_SECRETS`) and,
//! when granted, the encryption master key. It never reads a secrets file, a
//! vault or the profile's secrets source, and it has no pepper, manifest seed,
//! signing key or licence secret to hand out: asking it for the seed is an
//! error, as is asking for the master key when none was granted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::Utc;
use systemprompt_manifest::paths::constants::env_vars::CUSTOM_SECRETS;
use systemprompt_models::subprocess::ProviderSecret;
use systemprompt_traits::{
    ExtensionSecret, ReloadOutcome, SECRET_SEED_BYTES, SecretStore, SecretStoreError, SecretsStatus,
};
use zeroize::Zeroizing;

use super::SecretsBootstrapError;
use crate::bootstrap::master_key::decode_master_key;
use crate::config_loader::ConfigCredentials;

const ENCRYPTION_MASTER_KEY_ENV: &str = "ENCRYPTION_MASTER_KEY";
const SOURCE: &str = "grants";

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum GrantedSecretsError {
    #[error("DATABASE_URL is not in the child environment")]
    DatabaseUrlMissing,
    #[error("no manifest signing seed is held by a spawned server")]
    SeedNotHeld,
    #[error("the encryption master key was not granted to this server")]
    MasterKeyNotGranted,
}

/// A spawned server's granted secrets, keyed by lowercase secret name.
pub struct GrantedSecrets {
    credentials: ConfigCredentials,
    secrets: BTreeMap<String, Zeroizing<String>>,
    initialised_at: chrono::DateTime<Utc>,
}

impl std::fmt::Debug for GrantedSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrantedSecrets")
            .field("granted", &self.secrets.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl GrantedSecrets {
    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, GrantedSecretsError> {
        let present = |name: &str| lookup(name).filter(|value| !value.is_empty());
        let mut secrets = BTreeMap::new();
        for provider in ProviderSecret::ALL {
            if let Some(value) = present(provider.env_name()) {
                secrets.insert(provider.as_str().to_owned(), Zeroizing::new(value));
            }
        }
        if let Some(names) = present(CUSTOM_SECRETS) {
            for name in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                if let Some(value) = present(name) {
                    secrets.insert(name.to_ascii_lowercase(), Zeroizing::new(value));
                }
            }
        }
        let credentials = ConfigCredentials {
            database_url: present("DATABASE_URL").ok_or(GrantedSecretsError::DatabaseUrlMissing)?,
            database_write_url: present("DATABASE_WRITE_URL"),
            database_session_url: present("DATABASE_SESSION_URL"),
            github_token: secrets
                .get(ProviderSecret::Github.as_str())
                .map(|token| token.as_str().to_owned()),
        };
        Ok(Self {
            credentials,
            secrets,
            initialised_at: Utc::now(),
        })
    }

    #[must_use]
    pub const fn credentials(&self) -> &ConfigCredentials {
        &self.credentials
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.secrets
            .get(&name.to_ascii_lowercase())
            .map(|value| value.as_str())
    }

    #[must_use]
    pub fn granted_names(&self) -> Vec<String> {
        self.secrets.keys().cloned().collect()
    }

    fn master_key(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        let encoded = self.get(ENCRYPTION_MASTER_KEY_ENV).ok_or_else(|| {
            SecretStoreError::new(
                "reading the encryption master key",
                GrantedSecretsError::MasterKeyNotGranted,
            )
        })?;
        decode_master_key(encoded).map_err(|e: SecretsBootstrapError| {
            SecretStoreError::new("decoding the encryption master key", e)
        })
    }
}

#[async_trait::async_trait]
impl SecretStore for GrantedSecrets {
    fn secret(&self, name: &str) -> Option<ExtensionSecret> {
        self.get(name)
            .map(|value| ExtensionSecret::new(value.to_owned()))
    }

    fn key_names(&self) -> Result<Vec<String>, SecretStoreError> {
        Ok(self.granted_names())
    }

    fn manifest_signing_secret_seed(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        Err(SecretStoreError::new(
            "reading the manifest signing seed",
            GrantedSecretsError::SeedNotHeld,
        ))
    }

    fn encryption_master_key(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        self.master_key()
    }

    fn status(&self) -> SecretsStatus {
        SecretsStatus {
            source: SOURCE.to_owned(),
            generation: 0,
            initialised_at: self.initialised_at,
            last_reload_at: None,
            last_error: None,
        }
    }

    fn refresh_interval(&self) -> Option<Duration> {
        None
    }

    async fn reload(&self) -> Result<ReloadOutcome, SecretStoreError> {
        Ok(ReloadOutcome {
            generation: 0,
            changed_keys: Vec::new(),
            swapped: false,
        })
    }
}

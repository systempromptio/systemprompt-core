//! `HashiCorp` Vault / `OpenBao` KV v2 secrets provider.
//!
//! One KV v2 document holds the whole `secrets.json` shape. Individual entries
//! can be redirected with the profile's `keys:` map, which reads a named field
//! out of a different KV path — that is how identity material shared across
//! instances lives in one place while each instance keeps its own document.
//!
//! The Vault token never comes from profile YAML. It is read from the process
//! environment or a file, or minted by an `AppRole` / Kubernetes login, and is
//! held in [`zeroize::Zeroizing`] for the length of the fetch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod auth;
mod client;
mod error;
mod kv;

use std::collections::BTreeMap;

use systemprompt_models::profile::{VaultAuth, VaultKeyRef, VaultSecretsConfig};

use super::SecretsBootstrapError;
use super::provider::{SecretsDocument, SecretsProvider};
use client::VaultHttp;
pub use error::VaultError;

pub(super) type EnvLookup = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

pub struct VaultKvProvider {
    http: VaultHttp,
    auth: VaultAuth,
    mount: String,
    path: String,
    keys: BTreeMap<String, VaultKeyRef>,
    address: String,
    lookup_env: EnvLookup,
}

impl std::fmt::Debug for VaultKvProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultKvProvider")
            .field("address", &self.address)
            .field("mount", &self.mount)
            .field("path", &self.path)
            .field("auth", &self.auth.method_name())
            .finish_non_exhaustive()
    }
}

impl VaultKvProvider {
    pub fn from_config(
        cfg: &VaultSecretsConfig,
        lookup_env: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> Result<Self, VaultError> {
        Ok(Self {
            http: VaultHttp::new(cfg)?,
            auth: cfg.auth.clone(),
            mount: cfg.mount.clone(),
            path: cfg.path.clone(),
            keys: cfg.keys.clone(),
            address: cfg.address.clone(),
            lookup_env: Box::new(lookup_env),
        })
    }

    async fn fetch_document(&self) -> Result<SecretsDocument, VaultError> {
        let session = auth::login(&self.http, &self.auth, &self.lookup_env).await?;

        let base = kv::read_entry(&self.http, &session.token, &self.mount, &self.path).await?;
        tracing::debug!(
            address = %self.address,
            mount = %self.mount,
            path = %self.path,
            version = base.version,
            lease_duration = session.lease_duration,
            auth_method = self.auth.method_name(),
            "loaded secrets document from vault"
        );

        let mut document = SecretsDocument::new(base.fields);

        for (key, reference) in &self.keys {
            let entry =
                kv::read_entry(&self.http, &session.token, &self.mount, &reference.path).await?;
            let value = entry.fields.get(&reference.field).cloned().ok_or_else(|| {
                VaultError::Malformed {
                    message: format!(
                        "{}/{} has no field '{}' for override key '{key}'",
                        self.mount, reference.path, reference.field
                    ),
                }
            })?;
            document.merge_field(key.clone(), value);
        }

        Ok(document)
    }
}

impl SecretsProvider for VaultKvProvider {
    async fn fetch(&self) -> Result<SecretsDocument, SecretsBootstrapError> {
        self.fetch_document().await.map_err(Into::into)
    }

    fn describe(&self) -> String {
        format!(
            "vault {} ({}/{}, auth {})",
            self.address,
            self.mount,
            self.path,
            self.auth.method_name()
        )
    }
}

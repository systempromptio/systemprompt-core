//! OS credential-store backend for the OAuth `client_secret`.
//!
//! A [`SecretStore`] is resolved at the bridge's composition root: the OS
//! keystore (Keychain / Credential Manager / Secret Service / keyutils) when
//! one is usable, otherwise a map held by that store for the life of the
//! process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::PluginOAuthError;
use crate::config::keystore;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};
use systemprompt_identifiers::ClientId;

/// Which backend a [`SecretStore`] resolved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretBackend {
    Keyring,
    Memory,
}

/// Where the OAuth `client_secret` is kept for this process.
///
/// The memory backend is the runtime fallback for hosts with no usable OS
/// credential store (headless Linux, containers whose seccomp profile denies
/// keyutils): the secret lives only as long as the owning store and is
/// re-provisioned from the gateway on restart.
#[derive(Debug)]
pub struct SecretStore {
    backend: OnceLock<Backend>,
}

#[derive(Debug)]
enum Backend {
    Keyring,
    Memory(Mutex<HashMap<ClientId, String>>),
}

impl SecretStore {
    #[must_use]
    pub const fn resolve() -> Self {
        Self {
            backend: OnceLock::new(),
        }
    }

    fn backend_ref(&self) -> &Backend {
        self.backend.get_or_init(|| match keystore::default_store() {
            Ok(_) => Backend::Keyring,
            Err(e) => {
                tracing::warn!(
                    target: "bridge::auth::keystore",
                    error = %e,
                    "no OS credential store available; holding the OAuth client secret in memory \
                     for this process only. It is re-provisioned from the gateway on restart, so \
                     hooks keep working, but a second process (e.g. `doctor`) will report no \
                     client. Install a Secret Service provider (gnome-keyring), or allow kernel \
                     keyrings (docker: --security-opt seccomp=unconfined), to make it persistent."
                );
                Backend::Memory(Mutex::new(HashMap::new()))
            },
        })
    }

    #[must_use]
    pub fn backend(&self) -> SecretBackend {
        match self.backend_ref() {
            Backend::Keyring => SecretBackend::Keyring,
            Backend::Memory(_) => SecretBackend::Memory,
        }
    }

    pub(super) fn write(&self, client_id: &ClientId, secret: &str) -> Result<(), PluginOAuthError> {
        match self.backend_ref() {
            Backend::Keyring => keyring_entry(client_id)?
                .set_password(secret)
                .map_err(PluginOAuthError::Keyring),
            Backend::Memory(map) => {
                lock(map)?.insert(client_id.clone(), secret.to_owned());
                Ok(())
            },
        }
    }

    pub(super) fn read(&self, client_id: &ClientId) -> Result<Option<String>, PluginOAuthError> {
        match self.backend_ref() {
            Backend::Keyring => match keyring_entry(client_id)?.get_password() {
                Ok(s) => Ok(Some(s)),
                Err(keyring_core::Error::NoEntry) => Ok(None),
                Err(e) => Err(PluginOAuthError::Keyring(e)),
            },
            Backend::Memory(map) => Ok(lock(map)?.get(client_id).cloned()),
        }
    }

    pub(super) fn delete(&self, client_id: &ClientId) -> Result<(), PluginOAuthError> {
        match self.backend_ref() {
            Backend::Keyring => match keyring_entry(client_id)?.delete_credential() {
                Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(e) => Err(PluginOAuthError::Keyring(e)),
            },
            Backend::Memory(map) => {
                lock(map)?.remove(client_id);
                Ok(())
            },
        }
    }
}

fn lock(
    map: &Mutex<HashMap<ClientId, String>>,
) -> Result<MutexGuard<'_, HashMap<ClientId, String>>, PluginOAuthError> {
    map.lock()
        .map_err(|_poisoned| PluginOAuthError::MemoryStorePoisoned)
}

fn keyring_entry(client_id: &ClientId) -> Result<keyring_core::Entry, PluginOAuthError> {
    keyring_core::Entry::new(crate::brand::brand().keyring_service, client_id.as_str())
        .map_err(PluginOAuthError::Keyring)
}

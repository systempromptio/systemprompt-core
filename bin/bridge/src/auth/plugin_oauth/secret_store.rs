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
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
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
    backend: Backend,
}

#[derive(Debug)]
enum Backend {
    Keyring,
    Memory(Mutex<HashMap<ClientId, String>>),
}

impl SecretStore {
    #[must_use]
    pub fn resolve() -> Self {
        let backend = match install_store() {
            Ok(()) => Backend::Keyring,
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
        };
        Self { backend }
    }

    #[must_use]
    pub const fn backend(&self) -> SecretBackend {
        match self.backend {
            Backend::Keyring => SecretBackend::Keyring,
            Backend::Memory(_) => SecretBackend::Memory,
        }
    }

    pub(super) fn write(&self, client_id: &ClientId, secret: &str) -> Result<(), PluginOAuthError> {
        match &self.backend {
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
        match &self.backend {
            Backend::Keyring => match keyring_entry(client_id)?.get_password() {
                Ok(s) => Ok(Some(s)),
                Err(keyring_core::Error::NoEntry) => Ok(None),
                Err(e) => Err(PluginOAuthError::Keyring(e)),
            },
            Backend::Memory(map) => Ok(lock(map)?.get(client_id).cloned()),
        }
    }

    pub(super) fn delete(&self, client_id: &ClientId) -> Result<(), PluginOAuthError> {
        match &self.backend {
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

// Why: `keyring_core`'s default store is process global and set-once, so a
// store installed earlier in the process (by another `SecretStore`) is reused.
fn install_store() -> Result<(), PluginOAuthError> {
    if keyring_core::get_default_store().is_some() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    let store =
        apple_native_keyring_store::keychain::Store::new().map_err(PluginOAuthError::Keyring);
    #[cfg(target_os = "windows")]
    let store = windows_native_keyring_store::Store::new().map_err(PluginOAuthError::Keyring);
    #[cfg(all(unix, not(target_os = "macos")))]
    let store = linux_store();

    let store = store?;
    keyring_core::set_default_store(store);
    Ok(())
}

// Why: Headless Linux may lack Secret Service; Docker seccomp can also deny
// add_key.
#[cfg(all(unix, not(target_os = "macos")))]
fn linux_store() -> Result<std::sync::Arc<keyring_core::CredentialStore>, PluginOAuthError> {
    let dbus_err = match dbus_secret_service_keyring_store::Store::new() {
        Ok(store) => return Ok(store),
        Err(e) => e,
    };
    let store: std::sync::Arc<keyring_core::CredentialStore> =
        linux_keyutils_keyring_store::Store::new().map_err(|keyutils| {
            PluginOAuthError::NoCredentialStore {
                secret_service: dbus_err.to_string(),
                keyutils,
            }
        })?;
    probe_store(&store).map_err(|keyutils| PluginOAuthError::NoCredentialStore {
        secret_service: dbus_err.to_string(),
        keyutils,
    })?;
    tracing::warn!(
        target: "bridge::auth::keystore",
        secret_service_error = %dbus_err,
        "no Secret Service provider on this host (headless Linux?); using the kernel keyutils \
         keyring. The OAuth client secret will not survive a reboot and is re-provisioned \
         automatically."
    );
    Ok(store)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn probe_store(
    store: &std::sync::Arc<keyring_core::CredentialStore>,
) -> Result<(), keyring_core::Error> {
    let service = format!("{}-probe", crate::brand::brand().keyring_service);
    let entry = store.build(&service, "probe", None)?;
    entry.set_password("probe")?;
    entry.get_password()?;
    if let Err(e) = entry.delete_credential() {
        tracing::debug!(
            target: "bridge::auth::keystore",
            error = %e,
            "credential-store probe entry could not be removed; it is overwritten on each probe"
        );
    }
    Ok(())
}

fn keyring_entry(client_id: &ClientId) -> Result<keyring_core::Entry, PluginOAuthError> {
    keyring_core::Entry::new(crate::brand::brand().keyring_service, client_id.as_str())
        .map_err(PluginOAuthError::Keyring)
}

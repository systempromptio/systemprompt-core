//! OS credential-store backend for the OAuth `client_secret`.
//!
//! Resolves a keystore once per process (Keychain / Credential Manager / Secret
//! Service / keyutils) and falls back to an in-memory map when none is usable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::PluginOAuthError;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};
use systemprompt_identifiers::ClientId;

/// Which backend `write_secret`/`read_secret`/`delete_secret` are using.
///
/// Resolved exactly once, because `keyring_core`'s default store is process
/// global and set-once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretBackend {
    Keyring,
    Memory,
}

impl SecretBackend {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::Memory => "memory",
        }
    }
}

static BACKEND: OnceLock<SecretBackend> = OnceLock::new();
static MEMORY_SECRETS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn credential_backend() -> SecretBackend {
    resolve_backend()
}

fn resolve_backend() -> SecretBackend {
    if let Some(known) = BACKEND.get() {
        return *known;
    }
    let backend = match install_store() {
        Ok(()) => SecretBackend::Keyring,
        Err(e) => {
            tracing::warn!(
                target: "bridge::auth::keystore",
                error = %e,
                "no OS credential store available; holding the OAuth client secret in memory for \
                 this process only. It is re-provisioned from the gateway on restart, so hooks \
                 keep working, but a second process (e.g. `doctor`) will report no client. \
                 Install a Secret Service provider (gnome-keyring), or allow kernel keyrings \
                 (docker: --security-opt seccomp=unconfined), to make it persistent."
            );
            SecretBackend::Memory
        },
    };
    *BACKEND.get_or_init(|| backend)
}

fn install_store() -> Result<(), PluginOAuthError> {
    if keyring_core::get_default_store().is_some() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new()
        .map_err(|e| PluginOAuthError::Keyring(e.to_string()));
    #[cfg(target_os = "windows")]
    let store = windows_native_keyring_store::Store::new()
        .map_err(|e| PluginOAuthError::Keyring(e.to_string()));
    #[cfg(all(unix, not(target_os = "macos")))]
    let store = linux_store();

    let store = store?;
    keyring_core::set_default_store(store);
    Ok(())
}

// Why: headless Linux has no Secret Service provider, so the D-Bus store fails
// even with a session bus; Docker's seccomp denies `add_key`, so probe by
// writing.
#[cfg(all(unix, not(target_os = "macos")))]
fn linux_store() -> Result<std::sync::Arc<keyring_core::CredentialStore>, PluginOAuthError> {
    let dbus_err = match dbus_secret_service_keyring_store::Store::new() {
        Ok(store) => return Ok(store),
        Err(e) => e.to_string(),
    };
    let store: std::sync::Arc<keyring_core::CredentialStore> =
        linux_keyutils_keyring_store::Store::new().map_err(|e| {
            PluginOAuthError::Keyring(format!(
                "no usable credential store: secret-service ({dbus_err}), keyutils ({e})"
            ))
        })?;
    probe_store(&store).map_err(|e| {
        PluginOAuthError::Keyring(format!(
            "no usable credential store: secret-service ({dbus_err}), \
             keyutils built but is unusable ({e})"
        ))
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
fn probe_store(store: &std::sync::Arc<keyring_core::CredentialStore>) -> Result<(), String> {
    let service = format!("{}-probe", crate::brand::brand().keyring_service);
    let entry = store
        .build(&service, "probe", None)
        .map_err(|e| format!("build probe entry: {e}"))?;
    entry
        .set_password("probe")
        .map_err(|e| format!("probe write: {e}"))?;
    entry
        .get_password()
        .map_err(|e| format!("probe read: {e}"))?;
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
        .map_err(|e| PluginOAuthError::Keyring(e.to_string()))
}

pub(super) fn write_secret(client_id: &ClientId, secret: &str) -> Result<(), PluginOAuthError> {
    match resolve_backend() {
        SecretBackend::Keyring => keyring_entry(client_id)?
            .set_password(secret)
            .map_err(|e| PluginOAuthError::Keyring(e.to_string())),
        SecretBackend::Memory => {
            memory_secrets()?.insert(client_id.as_str().to_owned(), secret.to_owned());
            Ok(())
        },
    }
}

pub(super) fn read_secret(client_id: &ClientId) -> Result<Option<String>, PluginOAuthError> {
    match resolve_backend() {
        SecretBackend::Keyring => match keyring_entry(client_id)?.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(PluginOAuthError::Keyring(e.to_string())),
        },
        SecretBackend::Memory => Ok(memory_secrets()?.get(client_id.as_str()).cloned()),
    }
}

pub(super) fn delete_secret(client_id: &ClientId) {
    let outcome = match resolve_backend() {
        SecretBackend::Keyring => {
            keyring_entry(client_id).and_then(|e| match e.delete_credential() {
                Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(e) => Err(PluginOAuthError::Keyring(e.to_string())),
            })
        },
        SecretBackend::Memory => memory_secrets().map(|mut m| {
            m.remove(client_id.as_str());
        }),
    };
    if let Err(e) = outcome {
        tracing::warn!(
            target: "bridge::auth::keystore",
            backend = resolve_backend().as_str(),
            error = %e,
            "could not delete the stored OAuth client secret; it will be overwritten on the next \
             provision"
        );
    }
}

fn memory_secrets()
-> Result<std::sync::MutexGuard<'static, HashMap<String, String>>, PluginOAuthError> {
    MEMORY_SECRETS.lock().map_err(|_poisoned| {
        PluginOAuthError::Keyring("in-memory secret store lock was poisoned".to_owned())
    })
}

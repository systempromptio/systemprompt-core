//! The process default OS credential store.
//!
//! Every keyring consumer in the bridge (the PAT and the plugin OAuth
//! `client_secret`) resolves its entries through `keyring_core`'s process
//! default store. [`default_store`] installs the platform store when none is
//! set yet and returns the one in place otherwise, so a store installed
//! earlier in the process is the store every consumer sees.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use keyring_core::CredentialStore;

/// Why no OS credential store could be installed as the process default.
#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    #[error("keyring: {0}")]
    Native(#[source] keyring_core::Error),
    #[error("no usable credential store: secret-service ({secret_service}), keyutils ({keyutils})")]
    NoCredentialStore {
        secret_service: String,
        #[source]
        keyutils: keyring_core::Error,
    },
}

// Why: `keyring_core`'s default store is process global, so a store installed
// earlier in the process is reused rather than replaced.
pub(crate) fn default_store() -> Result<Arc<CredentialStore>, KeystoreError> {
    if let Some(store) = keyring_core::get_default_store() {
        return Ok(store);
    }
    keyring_core::set_default_store(platform_store()?);
    keyring_core::get_default_store()
        .ok_or(KeystoreError::Native(keyring_core::Error::NoDefaultStore))
}

#[cfg(target_os = "macos")]
fn platform_store() -> Result<Arc<CredentialStore>, KeystoreError> {
    let store: Arc<CredentialStore> =
        apple_native_keyring_store::keychain::Store::new().map_err(KeystoreError::Native)?;
    Ok(store)
}

#[cfg(target_os = "windows")]
fn platform_store() -> Result<Arc<CredentialStore>, KeystoreError> {
    let store: Arc<CredentialStore> =
        windows_native_keyring_store::Store::new().map_err(KeystoreError::Native)?;
    Ok(store)
}

// Why: Headless Linux may lack Secret Service; Docker seccomp can also deny
// add_key.
#[cfg(all(unix, not(target_os = "macos")))]
fn platform_store() -> Result<Arc<CredentialStore>, KeystoreError> {
    let dbus_err = match dbus_secret_service_keyring_store::Store::new() {
        Ok(store) => {
            let store: Arc<CredentialStore> = store;
            return Ok(store);
        },
        Err(e) => e,
    };
    let store: Arc<CredentialStore> =
        linux_keyutils_keyring_store::Store::new().map_err(|keyutils| {
            KeystoreError::NoCredentialStore {
                secret_service: dbus_err.to_string(),
                keyutils,
            }
        })?;
    probe(&store, "probe").map_err(|keyutils| KeystoreError::NoCredentialStore {
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

// Why: Linux has a second home for a credential the Secret Service refuses (a
// locked collection in an SSH or headless session is common), so the PAT
// moves to the profile file there; the macOS keychain and Windows Credential
// Manager have no alternative, so a refusal is the sign-in's error.
#[must_use]
pub(crate) const fn refusal_falls_back_to_file() -> bool {
    cfg!(all(unix, not(target_os = "macos")))
}

pub(crate) fn probe(
    store: &Arc<CredentialStore>,
    account: &str,
) -> Result<(), keyring_core::Error> {
    let service = format!("{}-probe", crate::brand::brand().keyring_service);
    let entry = store.build(&service, account, None)?;
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

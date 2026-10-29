//! Where a profile's PAT lives: the OS credential store, or a 0600 file in the
//! profile directory on a Linux host with no Secret Service.
//!
//! The store is chosen once, when the PAT is written, from what the platform
//! offers, and recorded as `pat.store` in the profile config. Reads follow the
//! recorded store and never fall back to the other one: a keyring entry that
//! has gone missing is a missing credential, not a cue to look for a file.
//! Linux's kernel keyutils store does not qualify — it forgets the PAT on
//! reboot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::ActiveProfile;
use crate::ids::PatToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatStore {
    Keyring,
    File,
}

impl PatStore {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::File => "file",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("OS credential store: {0}")]
    Keyring(#[source] keyring_core::Error),
    #[error("the OS credential store returned a different PAT than was written")]
    ReadBackMismatch,
    #[error("{action} {path}: {source}")]
    File {
        action: &'static str,
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the stored PAT is empty")]
    Empty,
}

fn account(profile: &ActiveProfile) -> String {
    format!("pat:{}", profile.id)
}

#[cfg(target_os = "macos")]
fn os_store() -> Result<Arc<keyring_core::CredentialStore>, keyring_core::Error> {
    let store: Arc<keyring_core::CredentialStore> =
        apple_native_keyring_store::keychain::Store::new()?;
    Ok(store)
}

#[cfg(target_os = "windows")]
fn os_store() -> Result<Arc<keyring_core::CredentialStore>, keyring_core::Error> {
    let store: Arc<keyring_core::CredentialStore> = windows_native_keyring_store::Store::new()?;
    Ok(store)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn os_store() -> Result<Arc<keyring_core::CredentialStore>, keyring_core::Error> {
    let store: Arc<keyring_core::CredentialStore> =
        dbus_secret_service_keyring_store::Store::new()?;
    Ok(store)
}

fn entry(profile: &ActiveProfile) -> Result<keyring_core::Entry, CredentialError> {
    let store = os_store().map_err(CredentialError::Keyring)?;
    store
        .build(
            crate::brand::brand().keyring_service,
            &account(profile),
            None,
        )
        .map_err(CredentialError::Keyring)
}

#[must_use]
pub fn select_store() -> PatStore {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        return PatStore::Keyring;
    }
    match os_store().and_then(|store| probe(&store)) {
        Ok(()) => PatStore::Keyring,
        Err(e) => {
            tracing::info!(
                target: "bridge::auth::keystore",
                error = %e,
                "no Secret Service provider; the PAT is kept in a 0600 file in the gateway profile"
            );
            PatStore::File
        },
    }
}

fn probe(store: &Arc<keyring_core::CredentialStore>) -> Result<(), keyring_core::Error> {
    let service = format!("{}-probe", crate::brand::brand().keyring_service);
    let entry = store.build(&service, "pat-probe", None)?;
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

pub fn write(profile: &ActiveProfile, store: PatStore, token: &str) -> Result<(), CredentialError> {
    let token = token.trim();
    match store {
        PatStore::Keyring => {
            let entry = entry(profile)?;
            entry
                .set_password(token)
                .map_err(CredentialError::Keyring)?;
            let back = entry.get_password().map_err(CredentialError::Keyring)?;
            if back != token {
                return Err(CredentialError::ReadBackMismatch);
            }
            Ok(())
        },
        PatStore::File => {
            let path = profile.pat_file();
            crate::fsutil::atomic_write_0600(&path, token.as_bytes()).map_err(|source| {
                CredentialError::File {
                    action: "write and verify",
                    path,
                    source,
                }
            })
        },
    }
}

pub fn read(profile: &ActiveProfile, store: PatStore) -> Result<Option<PatToken>, CredentialError> {
    let value = match store {
        PatStore::Keyring => match entry(profile)?.get_password() {
            Ok(value) => Some(value),
            Err(keyring_core::Error::NoEntry) => None,
            Err(e) => return Err(CredentialError::Keyring(e)),
        },
        PatStore::File => {
            let path = profile.pat_file();
            Some(
                std::fs::read_to_string(&path).map_err(|source| CredentialError::File {
                    action: "read",
                    path,
                    source,
                })?,
            )
        },
    };
    value
        .map(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                Err(CredentialError::Empty)
            } else {
                Ok(PatToken::new(trimmed))
            }
        })
        .transpose()
}

pub fn delete(profile: &ActiveProfile, store: PatStore) -> Result<(), CredentialError> {
    match store {
        PatStore::Keyring => match entry(profile)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(CredentialError::Keyring(e)),
        },
        PatStore::File => {
            let path = profile.pat_file();
            crate::fsutil::remove_verified(&path).map_err(|source| CredentialError::File {
                action: "remove",
                path,
                source,
            })
        },
    }
}

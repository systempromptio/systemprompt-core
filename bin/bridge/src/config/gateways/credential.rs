//! Where a profile's PAT lives: the process default OS credential store, or a
//! 0600 file in the profile directory.
//!
//! The store is chosen once, when the PAT is written, and recorded as
//! `pat.store` in the profile config. A default store that keeps credentials
//! until they are deleted is used when a probe entry round-trips through it.
//! When the probe is refused, Linux keeps the PAT in the file (a locked Secret
//! Service collection is common in SSH and headless sessions), while macOS and
//! Windows fail the sign-in, since the keychain and Credential Manager have no
//! alternative. The file is also chosen when the default store forgets
//! credentials (Linux kernel keyutils, a process-only store) or no store
//! exists. Reads follow the recorded store and never fall back to the other
//! one: a keyring entry that has gone missing is a missing credential, not a
//! cue to look for a file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use keyring_core::CredentialPersistence;
use serde::{Deserialize, Serialize};

use super::ActiveProfile;
use crate::config::keystore;
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
    #[error("OS credential store: {0}")]
    Keystore(#[source] keystore::KeystoreError),
    #[error(
        "the OS credential store refused to save the sign-in; unlock it or allow access when \
         prompted, then sign in again ({0})"
    )]
    StoreRefused(#[source] keyring_core::Error),
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

fn entry(profile: &ActiveProfile) -> Result<keyring_core::Entry, CredentialError> {
    let store = keystore::default_store().map_err(CredentialError::Keystore)?;
    store
        .build(
            crate::brand::brand().keyring_service,
            &account(profile),
            None,
        )
        .map_err(CredentialError::Keyring)
}

pub fn select_store() -> Result<PatStore, CredentialError> {
    let store = match keystore::default_store() {
        Ok(store) => store,
        Err(e @ keystore::KeystoreError::NoCredentialStore { .. }) => {
            return Ok(file_store(&e.to_string()));
        },
        Err(e) => return Err(CredentialError::Keystore(e)),
    };
    if let Some(lifetime) = forgetful(&store.persistence()) {
        return Ok(file_store(&format!(
            "the OS credential store keeps credentials only {lifetime}"
        )));
    }
    if let Err(e) = keystore::probe(&store, "pat-probe") {
        if keystore::refusal_falls_back_to_file() {
            tracing::warn!(
                target: "bridge::auth::keystore",
                error = %e,
                "the Secret Service refused a test entry; the PAT is kept in a 0600 file in the \
                 gateway profile"
            );
            return Ok(PatStore::File);
        }
        tracing::warn!(
            target: "bridge::auth::keystore",
            error = %e,
            "the OS credential store refused a test entry; the PAT was not stored"
        );
        return Err(CredentialError::StoreRefused(e));
    }
    tracing::info!(
        target: "bridge::auth::keystore",
        store = %store.vendor(),
        "the PAT is kept in the OS credential store"
    );
    Ok(PatStore::Keyring)
}

fn file_store(reason: &str) -> PatStore {
    tracing::info!(
        target: "bridge::auth::keystore",
        reason,
        "no persistent OS credential store; the PAT is kept in a 0600 file in the gateway profile"
    );
    PatStore::File
}

const fn forgetful(persistence: &CredentialPersistence) -> Option<&'static str> {
    match persistence {
        CredentialPersistence::UntilDelete => None,
        CredentialPersistence::EntryOnly => Some("for the life of one entry"),
        CredentialPersistence::ProcessOnly => Some("for the life of the process"),
        CredentialPersistence::UntilLogout => Some("until logout"),
        CredentialPersistence::UntilReboot => Some("until reboot"),
        _ => Some("for an unspecified lifetime"),
    }
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

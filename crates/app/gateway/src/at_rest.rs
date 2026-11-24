//! The gateway's at-rest material: the accounting journal and the at-rest
//! cipher.
//!
//! Both are keyed from the deployment's `encryption_master_key`, which only
//! the sealed secret store holds, so the `gateway` member reads the key once in
//! its own provide and provides [`GatewayAtRest`] **sealed**, conferred on
//! `gateway-http` under the `gateway-at-rest` row. No route state reads the
//! secret store; the gateway routes and the boot-time journal recovery read
//! this value instead. It opens the journal under the storage data directory
//! and seals and opens values under the master key; it reads no other secret.
//!
//! A host that does not hold the key (a child host the key was not granted
//! to, a CLI without the secret) still resolves: its cipher is withheld and
//! opening the journal answers [`GatewayAtRestError::KeyNotHeld`], so the
//! gateway surface fails to build there exactly as it did when it read the
//! secret store itself. The journal directory is created only when the
//! journal is opened, never at kernel resolve. It is opened once and every
//! reader (the surface, the boot-time recovery) shares that one journal: a
//! process owns one segment set, and recovery settles only what it owns or
//! adopts from a process that has exited.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use systemprompt_security::at_rest::AtRestCipher;
use systemprompt_traits::SECRET_SEED_BYTES;

use crate::audit::journal::GatewayJournal;
use crate::error::GatewayAuditError;

#[derive(Debug, thiserror::Error)]
pub enum GatewayAtRestError {
    #[error("the encryption master key is not held on this host")]
    KeyNotHeld,
    #[error("no storage data directory is configured on this host")]
    NoStateDir,
    #[error("the gateway journal could not be opened")]
    Journal(#[source] GatewayAuditError),
    #[error("the gateway journal handle is poisoned")]
    Poisoned,
}

/// The accounting journal and the at-rest cipher, keyed once from the
/// deployment's master key.
#[derive(Clone)]
pub struct GatewayAtRest {
    state_dir: Option<PathBuf>,
    key: Option<[u8; SECRET_SEED_BYTES]>,
    journal: Arc<Mutex<Option<GatewayJournal>>>,
}

impl std::fmt::Debug for GatewayAtRest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayAtRest")
            .field("state_dir", &self.state_dir)
            .field("key_held", &self.key.is_some())
            .finish_non_exhaustive()
    }
}

impl GatewayAtRest {
    #[must_use]
    pub fn new(state_dir: Option<PathBuf>, key: Option<[u8; SECRET_SEED_BYTES]>) -> Self {
        Self {
            state_dir,
            key,
            journal: Arc::new(Mutex::new(None)),
        }
    }

    pub fn journal(&self) -> Result<GatewayJournal, GatewayAtRestError> {
        let key = self.key.ok_or(GatewayAtRestError::KeyNotHeld)?;
        let state_dir = self
            .state_dir
            .as_ref()
            .ok_or(GatewayAtRestError::NoStateDir)?;
        let mut opened = self
            .journal
            .lock()
            .map_err(|_poisoned| GatewayAtRestError::Poisoned)?;
        if let Some(journal) = opened.as_ref() {
            return Ok(journal.clone());
        }
        let journal = GatewayJournal::open(state_dir, key).map_err(GatewayAtRestError::Journal)?;
        *opened = Some(journal.clone());
        drop(opened);
        Ok(journal)
    }

    #[must_use]
    pub fn cipher(&self) -> AtRestCipher {
        self.key
            .map_or_else(AtRestCipher::withheld, AtRestCipher::from_master_key)
    }
}

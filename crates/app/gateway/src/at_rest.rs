//! The gateway's at-rest material: the accounting journal and the at-rest
//! cipher.
//!
//! Both are keyed from the deployment's `encryption_master_key`, which only
//! the sealed secret store holds, so the `gateway` member reads the key once in
//! its own provide and provides [`GatewayAtRest`] **sealed**, conferred on
//! `gateway-http` under the `gateway-at-rest` row. No route state reads the
//! secret store; the gateway routes and the boot-time journal recovery read
//! this value instead. It opens the journal under the storage data directory,
//! keyed for the `gateway` member under the journal's own label, and seals
//! and opens personal provider keys under the `ai` member's at-rest cipher
//! (the owner of `user_provider_credentials`); nothing is sealed under the
//! master key itself, and it reads no other secret.
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

use systemprompt_identifiers::ExtensionId;
use systemprompt_security::at_rest::{AtRestCipher, MasterKey};

const GATEWAY_MEMBER: &str = "gateway";

use crate::audit::journal::{
    GatewayJournal, JOURNAL_DIR, JournalReseal, journal_cipher, reseal_segments,
};
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
    key: Option<MasterKey>,
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
    pub fn new(state_dir: Option<PathBuf>, key: Option<MasterKey>) -> Self {
        Self {
            state_dir,
            key,
            journal: Arc::new(Mutex::new(None)),
        }
    }

    pub fn journal(&self) -> Result<GatewayJournal, GatewayAtRestError> {
        let key = self.key.as_ref().ok_or(GatewayAtRestError::KeyNotHeld)?;
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

    pub fn reseal_journal(&self, apply: bool) -> Result<JournalReseal, GatewayAtRestError> {
        let master = self.key.as_ref().ok_or(GatewayAtRestError::KeyNotHeld)?;
        let state_dir = self
            .state_dir
            .as_ref()
            .ok_or(GatewayAtRestError::NoStateDir)?;
        let cipher = journal_cipher(master).map_err(GatewayAtRestError::Journal)?;
        reseal_segments(&state_dir.join(JOURNAL_DIR), &cipher, master, apply)
            .map_err(GatewayAtRestError::Journal)
    }

    #[must_use]
    pub fn cipher(&self) -> AtRestCipher {
        self.key
            .as_ref()
            .and_then(|key| {
                AtRestCipher::for_member(
                    key,
                    &ExtensionId::new(systemprompt_ai::repository::CREDENTIALS_OWNER),
                )
                .map_err(|error| {
                    tracing::warn!(error = %error, "the credentials cipher does not derive; sealed credentials are withheld");
                })
                .ok()
            })
            .unwrap_or_else(AtRestCipher::withheld)
    }
}

/// The gateway journal's segments as files `admin secrets reseal` re-keys.
#[derive(Debug)]
pub struct JournalFiles(pub Arc<GatewayAtRest>);

impl systemprompt_extension::SealedFiles for JournalFiles {
    fn owner(&self) -> ExtensionId {
        ExtensionId::new(GATEWAY_MEMBER)
    }

    fn label(&self) -> &'static str {
        JOURNAL_DIR
    }

    fn reseal(
        &self,
        apply: bool,
    ) -> Result<systemprompt_extension::SealedFilesReport, systemprompt_extension::SealedFilesError>
    {
        let report = self.0.reseal_journal(apply).map_err(|error| {
            systemprompt_extension::SealedFilesError {
                owner: self.owner(),
                source: Box::new(error),
            }
        })?;
        Ok(systemprompt_extension::SealedFilesReport {
            total: report.segments,
            current: report.current,
            resealed: if apply {
                report.resealed
            } else {
                report.pre_088
            },
            undecodable: report.undecodable,
            kept_unopened: report.frames_kept_unopened,
        })
    }
}

//! Memo of verified managed revision bundles, owned by the
//! [`crate::managed::ManagedRepository`] that loads them.
//!
//! A published revision is immutable and its bundle digest is checked before
//! it is stored, so an entry keyed by revision id and digest can be reused
//! for every later resolution without pulling its asset blobs from Postgres
//! again. The memo is bounded; when it fills it is cleared and refilled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use crate::managed::RevisionBundle;
use systemprompt_identifiers::ResourceRevisionId;

const CAPACITY: usize = 1024;

type Key = (ResourceRevisionId, String);

#[derive(Debug, Default)]
pub(crate) struct VerifiedRevisions {
    entries: Mutex<BTreeMap<Key, Arc<RevisionBundle>>>,
}

impl VerifiedRevisions {
    pub(crate) fn get(
        &self,
        revision: &ResourceRevisionId,
        digest: &str,
    ) -> Option<Arc<RevisionBundle>> {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&(revision.clone(), digest.to_owned()))
            .cloned()
    }

    pub(crate) fn insert(
        &self,
        revision: ResourceRevisionId,
        digest: String,
        bundle: Arc<RevisionBundle>,
    ) {
        let mut guard = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.len() >= CAPACITY {
            guard.clear();
        }
        guard.insert((revision, digest), bundle);
    }
}

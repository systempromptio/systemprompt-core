//! Process-wide memo of verified managed revision bundles.
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

use systemprompt_identifiers::ResourceRevisionId;
use crate::managed::RevisionBundle;

const CAPACITY: usize = 1024;

type Key = (ResourceRevisionId, String);

static VERIFIED: Mutex<BTreeMap<Key, Arc<RevisionBundle>>> = Mutex::new(BTreeMap::new());

pub(super) fn get(revision: &ResourceRevisionId, digest: &str) -> Option<Arc<RevisionBundle>> {
    VERIFIED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&(revision.clone(), digest.to_owned()))
        .cloned()
}

pub(super) fn insert(revision: ResourceRevisionId, digest: String, bundle: Arc<RevisionBundle>) {
    let mut guard = VERIFIED.lock().unwrap_or_else(PoisonError::into_inner);
    if guard.len() >= CAPACITY {
        guard.clear();
    }
    guard.insert((revision, digest), bundle);
}

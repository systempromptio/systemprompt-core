//! Per-entity declarations and writes: what [`super::ConfigPlane::sources`]
//! returns and what [`super::ConfigPlane::write`] receives.
//!
//! Each declared entity carries its own content hash and origin, so the apply
//! can keep a three-way-merge base per row rather than one hash per plane.
//! [`entity_hash`] is the one hash both sides use: a plane's declared value
//! and the database's canonical value compare equal exactly when their hashes
//! do.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::ChangeSource;

#[must_use]
pub fn entity_hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredEntity {
    pub value: String,
    pub hash: String,
    pub source: ChangeSource,
}

impl DeclaredEntity {
    #[must_use]
    pub fn new(value: String, source: ChangeSource) -> Self {
        Self {
            hash: entity_hash(&value),
            value,
            source,
        }
    }
}

pub type DeclaredEntities = BTreeMap<String, DeclaredEntity>;

/// One entity's outcome of a merge: the value to store (`None` deletes the
/// row) and the origin recorded with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEntity {
    pub value: Option<String>,
    pub source: ChangeSource,
}

pub type ResolvedEntities = BTreeMap<String, ResolvedEntity>;

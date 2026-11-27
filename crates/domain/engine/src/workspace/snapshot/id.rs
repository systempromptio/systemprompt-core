//! The content address of a snapshot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::SnapshotError;
use super::layout::is_hash;

/// The content address of one snapshot: the SHA-256 of its manifest, as 64
/// lowercase hex digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SnapshotId(String);

impl SnapshotId {
    pub(crate) const fn from_digest(digest: String) -> Self {
        Self(digest)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for SnapshotId {
    type Err = SnapshotError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if is_hash(value) {
            Ok(Self(value.to_owned()))
        } else {
            Err(SnapshotError::InvalidId)
        }
    }
}

impl TryFrom<String> for SnapshotId {
    type Error = SnapshotError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if is_hash(&value) {
            Ok(Self(value))
        } else {
            Err(SnapshotError::InvalidId)
        }
    }
}

impl From<SnapshotId> for String {
    fn from(id: SnapshotId) -> Self {
        id.0
    }
}

//! Who inserted each uploaded image of a live session.
//!
//! Owners are derived from the shared update log, which stores the editor
//! of every update: each update a replica imports (catch-up, rebuild or its
//! own apply) records its editor as the owner of every image reference the
//! update added. Later editors (someone who only types) never change an
//! owner, and only the recorded inserter's upload may place an image.
//!
//! A snapshot prunes the log it covers, so the owners travel inside the
//! snapshot blob as an envelope ([`ImageOwners::seal`],
//! [`ImageOwners::open`]): a magic line, the owners as JSON with its
//! length, then the engine snapshot. A snapshot written before owners
//! existed has no envelope and opens with none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

use crate::documents::model::{RichText, media_refs};

const MAGIC: &[u8] = b"SPOWNERS1\n";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageOwners {
    by_ref: BTreeMap<String, UserId>,
}

impl ImageOwners {
    pub fn record(&mut self, before: &RichText, after: &RichText, editor: &UserId) {
        let held = media_refs(before);
        for reference in media_refs(after) {
            if !held.contains(&reference) {
                self.by_ref
                    .entry(reference)
                    .or_insert_with(|| editor.clone());
            }
        }
    }

    #[must_use]
    pub fn owner(&self, reference: &str) -> Option<&UserId> {
        self.by_ref.get(reference)
    }

    #[must_use]
    pub fn seal(&self, snapshot: &[u8]) -> Vec<u8> {
        let owners = match serde_json::to_vec(self) {
            Ok(owners) => owners,
            Err(error) => {
                tracing::warn!(%error, "edit session image owners not serialisable; not sealed");
                return snapshot.to_vec();
            },
        };
        let Ok(len) = u32::try_from(owners.len()) else {
            tracing::warn!(
                bytes = owners.len(),
                "edit session image owners too large; not sealed"
            );
            return snapshot.to_vec();
        };
        let mut out = Vec::with_capacity(MAGIC.len() + 4 + owners.len() + snapshot.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&owners);
        out.extend_from_slice(snapshot);
        out
    }

    #[must_use]
    pub fn open(blob: &[u8]) -> (Self, &[u8]) {
        let Some(rest) = blob.strip_prefix(MAGIC) else {
            return (Self::default(), blob);
        };
        let Some((len, rest)) = rest.split_first_chunk::<4>() else {
            return (Self::default(), blob);
        };
        let len = u32::from_be_bytes(*len) as usize;
        let Some((owners, snapshot)) = rest.split_at_checked(len) else {
            return (Self::default(), blob);
        };
        let owners = serde_json::from_slice(owners).unwrap_or_else(|error| {
            tracing::warn!(%error, "edit session image owners unreadable; starting with none");
            Self::default()
        });
        (owners, snapshot)
    }
}

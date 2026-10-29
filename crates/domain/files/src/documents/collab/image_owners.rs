//! Who inserted each uploaded image of a live session.
//!
//! Inserters are derived from the shared update log, which stores the
//! editor of every update: each update a replica imports (catch-up, rebuild
//! or its own apply) records its editor as an inserter of every image
//! reference the update added. Every inserter is kept, so an editor who
//! inserts a reference first never displaces the editor who uploaded it;
//! an image is placed only when one of its inserters uploaded the bytes
//! the reference hashes to. Later editors who only type never become
//! inserters.
//!
//! A snapshot prunes the log it covers, so the inserters travel inside the
//! snapshot blob as an envelope ([`ImageOwners::seal`],
//! [`ImageOwners::open`]): a magic line, the inserters as JSON with its
//! length, then the engine snapshot. A snapshot without an envelope opens
//! with no inserters; an envelope that cannot be read is an
//! [`ImageOwnersError`]. The single-inserter `SPOWNERS1` envelope is still
//! read: 0.75.0 wrote it into stored snapshots, so it is a persisted format
//! rather than a migration path, and only `SPOWNERS2` is written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

use super::CollabError;
use crate::documents::engine::{EngineDoc, ModelView};
use crate::documents::model::{RichText, media_refs};
use crate::documents::repository::CollabUpdateRow;

const MAGIC: &[u8] = b"SPOWNERS2\n";
const MAGIC_SINGLE: &[u8] = b"SPOWNERS1\n";

/// Why a snapshot's image-inserter envelope could not be opened.
#[derive(Debug, thiserror::Error)]
pub enum ImageOwnersError {
    #[error("the image owner envelope is truncated")]
    Truncated,
    #[error("the image owner envelope is not valid JSON")]
    Decode(#[source] serde_json::Error),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageOwners {
    by_ref: BTreeMap<String, BTreeSet<UserId>>,
}

#[derive(Deserialize)]
struct SingleOwners {
    by_ref: BTreeMap<String, UserId>,
}

impl ImageOwners {
    pub fn record(&mut self, before: &RichText, after: &RichText, editor: &UserId) {
        let held = media_refs(before);
        for reference in media_refs(after) {
            if !held.contains(&reference) {
                self.by_ref
                    .entry(reference)
                    .or_default()
                    .insert(editor.clone());
            }
        }
    }

    #[must_use]
    pub fn inserters(&self, reference: &str) -> Vec<UserId> {
        self.by_ref
            .get(reference)
            .map(|editors| editors.iter().cloned().collect())
            .unwrap_or_default()
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

    pub fn open(blob: &[u8]) -> Result<(Self, &[u8]), ImageOwnersError> {
        if let Some(rest) = blob.strip_prefix(MAGIC) {
            let (owners, snapshot) = envelope(rest)?;
            let owners = serde_json::from_slice(owners).map_err(ImageOwnersError::Decode)?;
            return Ok((owners, snapshot));
        }
        if let Some(rest) = blob.strip_prefix(MAGIC_SINGLE) {
            let (owners, snapshot) = envelope(rest)?;
            let single: SingleOwners =
                serde_json::from_slice(owners).map_err(ImageOwnersError::Decode)?;
            let by_ref = single
                .by_ref
                .into_iter()
                .map(|(reference, owner)| (reference, BTreeSet::from([owner])))
                .collect();
            return Ok((Self { by_ref }, snapshot));
        }
        Ok((Self::default(), blob))
    }
}

fn envelope(rest: &[u8]) -> Result<(&[u8], &[u8]), ImageOwnersError> {
    let (len, rest) = rest
        .split_first_chunk::<4>()
        .ok_or(ImageOwnersError::Truncated)?;
    let len = u32::from_be_bytes(*len) as usize;
    rest.split_at_checked(len)
        .ok_or(ImageOwnersError::Truncated)
}

pub(super) fn author_runs(rows: &[CollabUpdateRow]) -> usize {
    1 + rows
        .windows(2)
        .filter(|pair| pair[0].author_user_id != pair[1].author_user_id)
        .count()
}

pub(super) fn adds_media(before: &RichText, after: &RichText) -> bool {
    let held = media_refs(before);
    media_refs(after)
        .iter()
        .any(|reference| !held.contains(reference))
}

pub(super) fn attribute_runs(
    images: &mut ImageOwners,
    mut fork: Box<dyn EngineDoc>,
    mut before: ModelView,
    rows: &[CollabUpdateRow],
) -> Result<(), CollabError> {
    for run in rows.chunk_by(|a, b| a.author_user_id == b.author_user_id) {
        for row in run {
            fork.apply_update(&row.update)?;
        }
        let after = fork.export_model()?;
        if let (ModelView::RichText(prior), ModelView::RichText(next)) = (&before, &after) {
            images.record(prior, next, &run[0].author_user_id);
        }
        before = after;
    }
    Ok(())
}

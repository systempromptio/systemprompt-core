//! Idempotent writes: the digest a commit's op id is bound to, and the
//! replay of a stored acknowledgement.
//!
//! The digest covers the base revision and the body (the ops as JSON, or
//! the uploaded bytes), so the same edit sent over REST or MCP under one op
//! id replays, and a different request under a used op id is refused. An
//! acknowledgement replays only for the user who wrote it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sha2::{Digest, Sha256};
use systemprompt_identifiers::{DocumentOpId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_traits::RepositoryError;

use super::{CommitOutcome, DocumentService, LandedCommit};
use crate::documents::error::DocumentsError;
use crate::documents::models::StoredWriteAck;
use crate::documents::revision::CommitBody;

pub(super) fn request_digest(
    base: Option<&DocumentRevisionId>,
    body: &CommitBody,
) -> Result<String, DocumentsError> {
    let mut hasher = Sha256::new();
    hasher.update(base.map_or("", DocumentRevisionId::as_str).as_bytes());
    hasher.update([0]);
    match body {
        CommitBody::Ops(ops) => {
            hasher.update(b"ops");
            hasher.update(serde_json::to_vec(ops).map_err(RepositoryError::from)?);
        },
        CommitBody::Bytes(bytes) => {
            hasher.update(b"bytes");
            hasher.update(bytes);
        },
    }
    Ok(hex::encode(hasher.finalize()))
}

impl DocumentService {
    pub(super) async fn replay(
        &self,
        who: &AuthenticatedUser,
        op_id: &DocumentOpId,
        digest: &str,
        stored: StoredWriteAck,
    ) -> Result<CommitOutcome, DocumentsError> {
        if stored.user_id.as_ref() != Some(&who.id)
            || stored.request_sha256.as_deref() != Some(digest)
        {
            return Err(DocumentsError::OpIdMismatch {
                op_id: op_id.clone(),
            });
        }
        let revision = self
            .repos
            .revisions
            .find(&stored.ack.revision_id)
            .await?
            .ok_or_else(|| DocumentsError::not_found("revision", &stored.ack.revision_id))?;
        let merged = stored.ack.merged_from.is_some();
        let landed = Box::new(LandedCommit {
            revision,
            ack: stored.ack,
            projection: None,
            replayed: true,
            warnings: Vec::new(),
        });
        Ok(if merged {
            CommitOutcome::Merged(landed)
        } else {
            CommitOutcome::Committed(landed)
        })
    }
}

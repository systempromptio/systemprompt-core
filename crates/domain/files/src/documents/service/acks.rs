//! Idempotent writes: the digest a commit's op id is bound to, and the
//! replay of a stored acknowledgement.
//!
//! The digest covers the base revision, the body (the ops as JSON, or the
//! uploaded bytes), the message, the actor kind, the lock holder and the
//! comment policy. The origin and trace attribution are left out, so the
//! same edit sent over REST or MCP under one op id replays, and any other
//! request under a used op id is refused. An acknowledgement replays only
//! for the user who wrote it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sha2::{Digest, Sha256};
use systemprompt_identifiers::{DocumentCommentId, DocumentOpId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_traits::RepositoryError;

use super::{CommitOutcome, CommitRequest, DocumentService, LandedCommit};
use crate::documents::error::DocumentsError;
use crate::documents::models::StoredWriteAck;
use crate::documents::revision::CommitBody;

fn field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

pub(super) fn request_digest(request: &CommitRequest) -> Result<String, DocumentsError> {
    let mut hasher = Sha256::new();
    field(
        &mut hasher,
        request
            .base_revision
            .as_ref()
            .map_or("", DocumentRevisionId::as_str)
            .as_bytes(),
    );
    match &request.body {
        CommitBody::Ops(ops) => {
            field(&mut hasher, b"ops");
            field(
                &mut hasher,
                &serde_json::to_vec(ops).map_err(RepositoryError::from)?,
            );
        },
        CommitBody::Bytes(bytes) => {
            field(&mut hasher, b"bytes");
            field(&mut hasher, bytes);
        },
    }
    field(
        &mut hasher,
        request.message.as_deref().unwrap_or_default().as_bytes(),
    );
    field(&mut hasher, &[u8::from(request.message.is_some())]);
    field(
        &mut hasher,
        &serde_json::to_vec(&request.author.actor_kind).map_err(RepositoryError::from)?,
    );
    field(
        &mut hasher,
        request
            .lock_holder_id
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    field(&mut hasher, &[u8::from(request.lock_holder_id.is_some())]);
    field(&mut hasher, &[u8::from(request.comments.allow_detach)]);
    field(
        &mut hasher,
        request
            .comments
            .answering
            .as_ref()
            .map_or("", DocumentCommentId::as_str)
            .as_bytes(),
    );
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
        let document = self.load_document(&revision.document_id).await?;
        let projection = self.projected(&document, &revision).await?.ids;
        let merged = stored.ack.merged_from.is_some();
        let landed = Box::new(LandedCommit {
            revision,
            ack: stored.ack,
            projection,
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

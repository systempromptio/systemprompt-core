//! [`DocumentService::edit_live`]: a commit's op batch written into the
//! document's open live session instead of onto the head.
//!
//! The request is the commit the caller would otherwise make, so a live
//! write keeps a commit's guarantees. An op id already acknowledged answers
//! `None`, and the caller's commit replays the stored acknowledgement (or
//! refuses a different request as `op_id_mismatch`). An agent's batch is
//! charged against its edit rate like a commit and refunded unless it is
//! written. A written batch with an op id stores its acknowledgement against
//! the session's base revision, so a retry replays instead of applying the
//! batch twice; a retry racing the first call between its log write and its
//! acknowledgement can still apply twice.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;

use super::{CommitRequest, DocumentService, LiveEdit};
use crate::documents::error::DocumentsError;

impl DocumentService {
    /// Writes `request`'s ops into the document's open live session, or
    /// answers `None` when there is none (or its op id was already used),
    /// so the caller commits `request` to the head.
    #[cfg(feature = "collab")]
    pub async fn edit_live(
        &self,
        who: &AuthenticatedUser,
        request: &CommitRequest,
        trace: &TraceId,
    ) -> Result<Option<LiveEdit>, DocumentsError> {
        use crate::documents::revision::CommitBody;
        let Some(collab) = self.collab() else {
            return Ok(None);
        };
        let CommitBody::Ops(ops) = &request.body else {
            return Ok(None);
        };
        if let Some(op_id) = &request.op_id
            && self
                .repos
                .write_acks
                .find(&request.document_id, op_id)
                .await?
                .is_some()
        {
            return Ok(None);
        }
        let digest = super::acks::request_digest(request)?;
        let reserved = self.reserve_agent_edits(who, ops.len())?;
        let live = super::LiveEditRequest {
            document_id: request.document_id.clone(),
            ops: ops.clone(),
            media: request.media.clone(),
        };
        let written = collab
            .edit_live(who, live, trace)
            .await
            .map_err(|error| match error {
                crate::documents::collab::CollabError::Documents(error) => error,
                other => DocumentsError::LiveSession(Box::new(other)),
            })?;
        let Some(written) = written else {
            return Ok(None);
        };
        if written.seq.is_some() {
            reserved.keep();
        }
        if let Some(op_id) = &request.op_id {
            self.acknowledge_live(who, request, op_id, &digest, &written)
                .await?;
        }
        Ok(Some(written))
    }

    #[cfg(feature = "collab")]
    async fn acknowledge_live(
        &self,
        who: &AuthenticatedUser,
        request: &CommitRequest,
        op_id: &systemprompt_identifiers::DocumentOpId,
        digest: &str,
        written: &LiveEdit,
    ) -> Result<(), DocumentsError> {
        use crate::documents::models::{AckClaim, WriteAck};
        use crate::documents::nodes::{created_nodes, written_nodes};
        use crate::documents::repository::DocumentWriteAckRepository;
        use systemprompt_traits::RepositoryError;
        let ack = WriteAck {
            revision_id: written.base_revision.clone(),
            parent_revision_id: None,
            merged_from: None,
            applied: written.applied,
            written: written_nodes(Some(&written.before), &written.after),
            created: created_nodes(Some(&written.before), &written.after),
        };
        let mut tx = self.repos.documents.begin().await?;
        let claim = AckClaim {
            document: &request.document_id,
            op_id,
            user: &who.id,
            request_sha256: digest,
            ack: &ack,
        };
        DocumentWriteAckRepository::claim(&mut tx, claim).await?;
        tx.commit()
            .await
            .map_err(|error| DocumentsError::Repository(RepositoryError::from(error)))?;
        Ok(())
    }

    #[cfg(not(feature = "collab"))]
    #[expect(
        clippy::unused_async,
        reason = "shares the signature of the collab build"
    )]
    pub async fn edit_live(
        &self,
        _who: &AuthenticatedUser,
        _request: &CommitRequest,
        _trace: &TraceId,
    ) -> Result<Option<LiveEdit>, DocumentsError> {
        Ok(None)
    }
}

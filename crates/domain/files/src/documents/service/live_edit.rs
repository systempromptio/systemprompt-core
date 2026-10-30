//! [`DocumentService::edit_live`]: a commit's op batch written into the
//! document's open live session instead of onto the head.
//!
//! The request is the commit the caller would otherwise make, so a live
//! write keeps a commit's guarantees. An op id is claimed before the batch
//! applies, acknowledged against the head, and released when the live write
//! fails or does not happen; an op id already claimed answers `None`, and
//! the caller's commit replays the stored acknowledgement (or refuses a
//! different request as `op_id_mismatch`). An agent's batch is charged
//! against its edit rate like a commit and refunded unless an update was
//! logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;

use super::{CommitRequest, DocumentService, LiveEdit};
use crate::documents::error::DocumentsError;

impl DocumentService {
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
        let claimed = match &request.op_id {
            Some(op_id) => match self.claim_live(who, request, op_id, trace).await? {
                Some(claimed) => Some(claimed),
                None => return Ok(None),
            },
            None => None,
        };
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
            });
        match written {
            Ok(Some(written)) => {
                if written.seq.is_some() {
                    reserved.keep();
                }
                Ok(Some(written))
            },
            other => {
                if let Some(op_id) = claimed {
                    self.repos
                        .write_acks
                        .release(&request.document_id, op_id, &who.id)
                        .await?;
                }
                other
            },
        }
    }

    #[cfg(feature = "collab")]
    async fn claim_live<'r>(
        &self,
        who: &AuthenticatedUser,
        request: &'r CommitRequest,
        op_id: &'r systemprompt_identifiers::DocumentOpId,
        trace: &TraceId,
    ) -> Result<Option<&'r systemprompt_identifiers::DocumentOpId>, DocumentsError> {
        use crate::documents::models::{AckClaim, WriteAck};
        use crate::documents::repository::DocumentWriteAckRepository;
        use crate::documents::revision::CommitBody;
        use systemprompt_models::documents::DocumentAction;
        use systemprompt_traits::RepositoryError;
        let document = self
            .authorized_document(who, &request.document_id, DocumentAction::Edit, trace)
            .await?;
        let Some(head) = document.head_revision_id else {
            return Ok(None);
        };
        let digest = super::acks::request_digest(request)?;
        let applied = match &request.body {
            CommitBody::Ops(ops) => ops.len(),
            CommitBody::Bytes(_) => 0,
        };
        let ack = WriteAck {
            revision_id: head,
            parent_revision_id: None,
            merged_from: None,
            applied,
            written: Vec::new(),
            created: Vec::new(),
        };
        let mut tx = self.repos.documents.begin().await?;
        let claim = AckClaim {
            document: &request.document_id,
            op_id,
            user: &who.id,
            request_sha256: &digest,
            ack: &ack,
        };
        let taken = DocumentWriteAckRepository::claim(&mut tx, claim).await?;
        tx.commit()
            .await
            .map_err(|error| DocumentsError::Repository(RepositoryError::from(error)))?;
        Ok(taken.is_none().then_some(op_id))
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

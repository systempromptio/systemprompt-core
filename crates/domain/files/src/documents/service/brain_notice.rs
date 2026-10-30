//! Durable brain facts the document store records inside the transaction of
//! the write that causes them: `document.revision.landed` when a landing
//! moves a document's head, and `document.comment.resolved` when a comment
//! is resolved. The fact goes to the outbox through the injected
//! [`DynBrainSink`](systemprompt_extension::DynBrainSink) on the write's own
//! connection, so a sink failure rolls the write back.
//!
//! When the sink reports itself unavailable (a server built or configured
//! without the brain) the write commits and nothing is recorded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use serde::Serialize;
use sqlx::PgConnection;
use systemprompt_extension::BrainSinkError;
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentCommentId, DocumentFolderId, DocumentId, DocumentNodeId,
    DocumentRevisionId, SessionId, TraceId, UserId,
};
use systemprompt_models::brain::{BrainEvent, BrainEventKind};
use systemprompt_models::documents::{ActorKind, RevisionOriginKind};

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentComment, DocumentRevision};

#[derive(Debug, Serialize)]
struct RevisionLandedPayload<'a> {
    document_id: &'a DocumentId,
    revision_id: &'a DocumentRevisionId,
    parent_revision_id: Option<&'a DocumentRevisionId>,
    actor_kind: ActorKind,
    origin_kind: RevisionOriginKind,
    author: &'a UserId,
    session_id: Option<&'a SessionId>,
    context_id: Option<&'a ContextId>,
    trace_id: Option<&'a TraceId>,
    folder_id: Option<&'a DocumentFolderId>,
    content_sha256: &'a ContentHash,
}

#[derive(Debug, Serialize)]
struct CommentResolvedPayload<'a> {
    document_id: &'a DocumentId,
    comment_id: &'a DocumentCommentId,
    comment_author: &'a UserId,
    resolved_by: &'a UserId,
    node_id: &'a DocumentNodeId,
    anchor_find: Option<&'a str>,
    anchor_nth: Option<i32>,
    head_revision_id: Option<&'a DocumentRevisionId>,
    answering_revision_id: Option<&'a DocumentRevisionId>,
    folder_id: Option<&'a DocumentFolderId>,
}

pub(super) struct ResolvedComment<'a> {
    pub(super) document: &'a Document,
    pub(super) comment: &'a DocumentComment,
    pub(super) resolved_by: &'a UserId,
    pub(super) answering: Option<&'a DocumentRevisionId>,
}

impl DocumentService {
    pub(super) async fn record_revision_landed(
        &self,
        conn: &mut PgConnection,
        document: &Document,
        revision: &DocumentRevision,
    ) -> Result<(), DocumentsError> {
        if !self.brain.is_available() {
            return Ok(());
        }
        let attribution = &revision.attribution;
        let payload = RevisionLandedPayload {
            document_id: &revision.document_id,
            revision_id: &revision.id,
            parent_revision_id: revision.parent_revision_id.as_ref(),
            actor_kind: revision.actor_kind,
            origin_kind: revision.origin_kind,
            author: &revision.author_user_id,
            session_id: attribution.session_id.as_ref(),
            context_id: attribution.context_id.as_ref(),
            trace_id: attribution.trace_id.as_ref(),
            folder_id: document.folder_id.as_ref(),
            content_sha256: &revision.content_sha256,
        };
        let occurred_at = revision.landed_at.unwrap_or_else(Utc::now);
        let event = BrainEvent::new(
            BrainEventKind::revision_landed(),
            revision.author_user_id.clone(),
            occurred_at,
        )
        .with_actor_is_agent(revision.actor_kind == ActorKind::Agent)
        .with_payload(serde_json::to_value(&payload).map_err(BrainSinkError::from)?);
        self.brain.emit(conn, &event).await?;
        Ok(())
    }

    pub(super) async fn record_comment_resolved(
        &self,
        conn: &mut PgConnection,
        resolved: ResolvedComment<'_>,
    ) -> Result<(), DocumentsError> {
        if !self.brain.is_available() {
            return Ok(());
        }
        let ResolvedComment {
            document,
            comment,
            resolved_by,
            answering,
        } = resolved;
        let payload = CommentResolvedPayload {
            document_id: &document.id,
            comment_id: &comment.id,
            comment_author: &comment.author_user_id,
            resolved_by,
            node_id: &comment.anchor.node,
            anchor_find: comment.anchor.find.as_deref(),
            anchor_nth: comment.anchor.nth,
            head_revision_id: document.head_revision_id.as_ref(),
            answering_revision_id: answering,
            folder_id: document.folder_id.as_ref(),
        };
        let event = BrainEvent::new(
            BrainEventKind::comment_resolved(),
            resolved_by.clone(),
            Utc::now(),
        )
        .with_payload(serde_json::to_value(&payload).map_err(BrainSinkError::from)?);
        self.brain.emit(conn, &event).await?;
        Ok(())
    }
}

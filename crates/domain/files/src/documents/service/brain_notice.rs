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
use sqlx::PgConnection;
use systemprompt_extension::BrainSinkError;
use systemprompt_identifiers::{DocumentRevisionId, UserId};
use systemprompt_models::brain::{
    BrainEvent, BrainEventKind, CommentAnchorRef, CommentResolved, RevisionLanded,
};
use systemprompt_models::documents::ActorKind;

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentComment, DocumentRevision};

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
        let payload = RevisionLanded {
            document_id: revision.document_id.clone(),
            revision_id: revision.id.clone(),
            parent_revision_id: revision.parent_revision_id.clone(),
            actor_kind: revision.actor_kind,
            origin_kind: revision.origin_kind,
            author: revision.author_user_id.clone(),
            session_id: attribution.session_id.clone(),
            context_id: attribution.context_id.clone(),
            trace_id: attribution.trace_id.clone(),
            task_id: attribution.task_id.clone(),
            on_behalf_of: attribution.on_behalf_of.clone(),
            folder_id: document.folder_id.clone(),
            content_sha256: revision.content_sha256.clone(),
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
        let payload = CommentResolved {
            document_id: document.id.clone(),
            comment_id: comment.id.clone(),
            comment_author: comment.author_user_id.clone(),
            resolved_by: resolved_by.clone(),
            anchor: CommentAnchorRef {
                node_id: comment.anchor.node.clone(),
                hash8: None,
                find: comment.anchor.find.clone(),
                nth: comment.anchor.nth,
            },
            head_revision_id: document.head_revision_id.clone(),
            answering_revision_id: answering.cloned(),
            folder_id: document.folder_id.clone(),
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

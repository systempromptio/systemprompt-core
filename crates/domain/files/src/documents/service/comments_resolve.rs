//! Resolving a comment, optionally naming the revision that answered it.
//!
//! The resolution, the answering revision and the durable
//! `document.comment.resolved` brain fact commit in one transaction on the
//! primary, so a sink failure leaves the comment open. The live
//! `DocumentEvents` frame is sent after commit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentCommentResolvedEvent};
use systemprompt_traits::RepositoryError;

use super::DocumentService;
use super::brain_notice::ResolvedComment;
use super::comments::ResolveCommentRequest;
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentComment;
use crate::documents::repository::DocumentCommentRepository;

impl DocumentService {
    pub async fn resolve_comment(
        &self,
        who: &AuthenticatedUser,
        request: ResolveCommentRequest,
        trace: &TraceId,
    ) -> Result<DocumentComment, DocumentsError> {
        let ResolveCommentRequest {
            document,
            comment,
            answering,
        } = request;
        let (document, comment) = (&document, &comment);
        let current = self
            .authorized_document(who, document, DocumentAction::Comment, trace)
            .await?;
        let open = self.comment_of(document, comment).await?;
        if let Some(revision) = &answering {
            self.resolve_revision(&current, Some(revision)).await?;
        }
        let mut tx = self.repos.documents.begin().await?;
        if !DocumentCommentRepository::resolve_in(&mut tx, document, comment, &who.id).await? {
            return Err(DocumentsError::CommentNotFound {
                document_id: document.clone(),
                comment_id: comment.clone(),
            });
        }
        if let Some(revision) = &answering {
            DocumentCommentRepository::set_answering_in(&mut tx, document, comment, revision)
                .await?;
        }
        let resolved = ResolvedComment {
            document: &current,
            comment: &open,
            resolved_by: &who.id,
            answering: answering.as_ref(),
        };
        self.record_comment_resolved(&mut tx, resolved).await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        let resolved = self.comment_of(document, comment).await?;
        let event = DocumentCommentResolvedEvent {
            document_id: document.clone(),
            comment_id: comment.clone(),
            resolved_by: who.id.clone(),
        };
        let to = self.recipients(&current).await;
        if let Err(error) = self.events.comment_resolved(&to, &event).await {
            tracing::warn!(document_id = %document, error = %error, "Comment event not relayed");
        }
        Ok(resolved)
    }
}

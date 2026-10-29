//! Comment anchors against a projection: whether an anchor still holds
//! (its node exists and, with a quote, the quote's `nth` occurrence is
//! still in the node's text), which open comments a commit would detach,
//! and settling them once the commit lands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentRevisionId};
use systemprompt_models::documents::{Projection, ProjectionNode};

use super::{CommentPolicy, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::CommentAnchor;

fn node_text(node: &ProjectionNode) -> String {
    let mut text = node.text.clone().unwrap_or_default();
    for child in &node.children {
        let inner = node_text(child);
        if !inner.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&inner);
        }
    }
    text
}

pub(super) fn anchor_holds(ids: &Projection, anchor: &CommentAnchor) -> bool {
    let Some(node) = ids.find(&anchor.node) else {
        return false;
    };
    anchor.find.as_deref().is_none_or(|find| {
        let nth = usize::try_from(anchor.nth.unwrap_or(0)).unwrap_or(0);
        node_text(node).matches(find).count() > nth
    })
}

impl DocumentService {
    pub(super) async fn affected_anchors(
        &self,
        document: &DocumentId,
        ids: &Projection,
        policy: &CommentPolicy,
    ) -> Result<Vec<DocumentCommentId>, DocumentsError> {
        let affected: Vec<DocumentCommentId> = self
            .repos
            .comments
            .list_for_document(document)
            .await?
            .into_iter()
            .filter(|c| {
                c.resolved_at.is_none() && c.deleted_at.is_none() && !anchor_holds(ids, &c.anchor)
            })
            .map(|c| c.id)
            .collect();
        if !affected.is_empty() && !policy.allow_detach {
            return Err(DocumentsError::AnchorsAffected { comments: affected });
        }
        Ok(affected)
    }

    pub(super) async fn settle_comments(
        &self,
        document: &DocumentId,
        revision: &DocumentRevisionId,
        detached: &[DocumentCommentId],
        policy: &CommentPolicy,
    ) -> Result<(), DocumentsError> {
        if !detached.is_empty() {
            self.repos.comments.detach(document, detached).await?;
        }
        if let Some(comment) = &policy.answering
            && !self
                .repos
                .comments
                .set_answering(document, comment, revision)
                .await?
        {
            tracing::warn!(
                document_id = %document,
                comment_id = %comment,
                "Answered comment is not on this document; the revision stands"
            );
        }
        Ok(())
    }
}

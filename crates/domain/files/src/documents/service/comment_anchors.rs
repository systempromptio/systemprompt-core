//! Comment anchors against a projection: whether an anchor still holds
//! (its node exists and, with a quote, the quote's `nth` occurrence is
//! still in the node's text; a cell range holds while its sheet node does,
//! since cells are positional), whether a new cell range anchor is well
//! placed, which open comments a commit would detach, and settling them
//! once the commit lands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentRevisionId};
use systemprompt_models::documents::{NodeKind, Projection, ProjectionNode};
use systemprompt_traits::RepositoryError;

use super::{CommentPolicy, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{CommentAnchor, CommentRange};
use crate::documents::repository::DocumentCommentRepository;

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
    if anchor.cells.is_some() {
        return node.kind == NodeKind::Sheet;
    }
    anchor.find.as_deref().is_none_or(|find| {
        let nth = usize::try_from(anchor.nth.unwrap_or(0)).unwrap_or(0);
        node_text(node).matches(find).count() > nth
    })
}

pub(super) fn check_cells(
    ids: &Projection,
    anchor: &CommentAnchor,
    range: Option<&CommentRange>,
) -> Result<(), DocumentsError> {
    if anchor.cells.is_none() {
        return Ok(());
    }
    if anchor.find.is_some() || anchor.nth.is_some() || range.is_some() {
        return Err(DocumentsError::InvalidComment {
            detail: "a cell range anchor takes no find, nth or range",
        });
    }
    if ids
        .find(&anchor.node)
        .is_some_and(|node| node.kind != NodeKind::Sheet)
    {
        return Err(DocumentsError::InvalidComment {
            detail: "a cell range anchors to a sheet node of a spreadsheet",
        });
    }
    Ok(())
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
        let Some(comment) = &policy.answering else {
            return Ok(());
        };
        let mut tx = self.repos.documents.begin().await?;
        let answered =
            DocumentCommentRepository::set_answering_in(&mut tx, document, comment, revision)
                .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        if !answered {
            tracing::warn!(
                document_id = %document,
                comment_id = %comment,
                "Answered comment is not on this document; the revision stands"
            );
        }
        Ok(())
    }
}

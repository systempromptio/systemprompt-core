//! A new document's first revision: landed in one transaction with its
//! node ids, as every later revision is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_models::auth::AuthenticatedUser;

use super::documents::NewContent;
use super::landing::{LandResult, Placement, Prepared};
use super::{CommentPolicy, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, NewRevision};
use crate::documents::nodes::CarryHints;

impl DocumentService {
    pub(super) async fn first_revision(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        hash: systemprompt_identifiers::ContentHash,
        content: &NewContent,
    ) -> Result<DocumentRevision, DocumentsError> {
        let author = &content.author;
        let next = self
            .project_paths(document.format, content.bytes.clone())
            .await?;
        let prepared = Prepared {
            revision: NewRevision {
                id: DocumentRevisionId::generate(),
                document_id: document.id.clone(),
                parent_revision_id: None,
                content_sha256: hash,
                author_user_id: who.id.clone(),
                actor_kind: author.actor_kind,
                origin: author.origin.clone(),
                message: content.message.clone(),
                edit_ops: content.ops.clone(),
                attribution: author.attribution.clone(),
            },
            next,
            previous: None,
            hints: CarryHints::default(),
            merged_from: None,
            applied: content.ops.as_ref().map_or(0, Vec::len),
        };
        let comments = CommentPolicy::detach();
        let placement = Placement {
            moves_head: true,
            lock_holder: None,
            op: None,
            settles: None,
            comments: &comments,
        };
        match self.land(document, prepared, placement).await? {
            LandResult::Landed(stored) => Ok(stored.revision),
            LandResult::HeadMoved | LandResult::Claimed(_) => {
                match self.repos.revisions.head(&document.id).await? {
                    Some(head) => Err(DocumentsError::Conflict {
                        document_id: document.id.clone(),
                        head: head.id,
                        base: None,
                    }),
                    None => Err(DocumentsError::not_found("document", &document.id)),
                }
            },
        }
    }
}

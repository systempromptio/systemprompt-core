//! A new document's first revision: landed in one transaction with its
//! node ids and its admitted images, as every later revision is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentRevisionId, DocumentTemplateId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentFormat, EditOp};

use super::landing::{LandResult, Placement, Prepared};
use super::{Author, CommentPolicy, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, NewRevision};
use crate::documents::nodes::CarryHints;

pub(super) struct NewContent {
    pub(super) folder: Option<DocumentFolderId>,
    pub(super) name: String,
    pub(super) format: DocumentFormat,
    pub(super) template_id: Option<DocumentTemplateId>,
    pub(super) bytes: Vec<u8>,
    pub(super) author: Author,
    pub(super) message: Option<String>,
    pub(super) ops: Option<Vec<EditOp>>,
    pub(super) mime_type: Option<String>,
    pub(super) vouched: Vec<String>,
}

impl DocumentService {
    pub(super) async fn first_revision(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        hash: systemprompt_identifiers::ContentHash,
        content: &NewContent,
    ) -> Result<DocumentRevision, DocumentsError> {
        let author = &content.author;
        let media = self
            .admissible_media(&who.id, document, &content.bytes, &content.vouched)
            .await?;
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
            media,
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

//! [`DocumentSourceContent`]: the core [`SourceContent`] for document and
//! comment sources.
//!
//! A document source reads the cited revision's id-addressed projection as
//! the document's owner under the `brain` system actor, so extraction sees
//! what the store holds rather than what the producing caller could see.
//! Every node carrying text becomes a [`SourceNode`], in document order, and
//! the source text is those nodes joined by newlines, so a cited span is
//! checkable against the node it names. A comment source reads the comment
//! and its live replies, without nodes. A document, revision or comment that
//! no longer exists reads as `None`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::brain::{SourceContent, SourceContentError};
use systemprompt_files::documents::agents::{DocumentActor, act_as};
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_files::documents::{DocumentService, DocumentsError};
use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::brain::{FactSource, SourceNode, SourceText};

use super::BRAIN_ACTOR;

/// Reads document revisions and comment threads as extraction source text.
#[derive(Debug, Clone)]
pub struct DocumentSourceContent {
    documents: Arc<DocumentService>,
    repos: Arc<DocumentRepositories>,
}

impl DocumentSourceContent {
    #[must_use]
    pub const fn new(documents: Arc<DocumentService>, repos: Arc<DocumentRepositories>) -> Self {
        Self { documents, repos }
    }

    async fn document(
        &self,
        document: &DocumentId,
        revision: &DocumentRevisionId,
    ) -> Result<Option<SourceText>, SourceContentError> {
        let Some(found) = self.repos.documents.find(document).await? else {
            return Ok(None);
        };
        let owner =
            AuthenticatedUser::new(found.owner_user_id, String::new(), String::new(), vec![]);
        let read = act_as(
            DocumentActor::System {
                name: BRAIN_ACTOR.to_owned(),
            },
            self.documents
                .project(&owner, document, Some(revision), &TraceId::system()),
        )
        .await;
        let projection = match read {
            Ok((_, projection)) => projection,
            Err(DocumentsError::NotFound { .. }) => return Ok(None),
            Err(error) => return Err(failed("document", error)),
        };
        let nodes: Vec<SourceNode> = projection
            .nodes()
            .filter_map(|node| {
                node.text.as_ref().map(|text| SourceNode {
                    node_id: node.id.clone(),
                    hash8: node.hash8.clone(),
                    text: text.clone(),
                })
            })
            .collect();
        let text = nodes
            .iter()
            .map(|node| node.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Some(SourceText { text, nodes }))
    }

    async fn comment(
        &self,
        document: &DocumentId,
        comment: &DocumentCommentId,
    ) -> Result<Option<SourceText>, SourceContentError> {
        let thread: Vec<String> = self
            .repos
            .comments
            .list_for_document(document)
            .await?
            .into_iter()
            .filter(|c| c.deleted_at.is_none())
            .filter(|c| c.id == *comment || c.parent_id.as_ref() == Some(comment))
            .map(|c| c.body)
            .collect();
        if thread.is_empty() {
            return Ok(None);
        }
        Ok(Some(SourceText {
            text: thread.join("\n\n"),
            nodes: Vec::new(),
        }))
    }
}

fn failed(kind: &str, error: DocumentsError) -> SourceContentError {
    SourceContentError::Other {
        source_kind: kind.to_owned(),
        source: Box::new(error),
    }
}

#[async_trait]
impl SourceContent for DocumentSourceContent {
    fn name(&self) -> &'static str {
        "documents"
    }

    fn handles(&self, source: &FactSource) -> bool {
        matches!(
            source,
            FactSource::Document { .. } | FactSource::Comment { .. }
        )
    }

    async fn fetch(&self, source: &FactSource) -> Result<Option<SourceText>, SourceContentError> {
        match source {
            FactSource::Document {
                document_id,
                revision_id,
                ..
            } => self.document(document_id, revision_id).await,
            FactSource::Comment {
                document_id,
                comment_id,
            } => self.comment(document_id, comment_id).await,
            _ => Ok(None),
        }
    }
}

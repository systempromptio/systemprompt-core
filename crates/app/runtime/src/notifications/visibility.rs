//! Per-reader visibility of document-store resources for notifications.
//!
//! A document is visible to a reader who holds any level on it, with its
//! excerpt only from `comment` up; a comment follows its document, and a
//! deleted comment or trashed document is hidden. A folder is hidden: the
//! access service decides folder levels only for the calling user, so no
//! folder notification is shown to a reader it cannot vouch for.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{ResourceVisibility, Visibility, VisibilityError};
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_files::documents::{DocumentAccessService, DocumentsError};
use systemprompt_identifiers::{DocumentCommentId, DocumentId, UserId};
use systemprompt_models::documents::AccessLevel;
use systemprompt_models::notifications::SubjectRef;

const DOCUMENT: &str = "document";
const FOLDER: &str = "folder";
const COMMENT: &str = "comment";

/// The [`ResourceVisibility`] for documents, folders and comments.
#[derive(Debug, Clone)]
pub struct DocumentVisibility {
    access: Arc<DocumentAccessService>,
    repos: Arc<DocumentRepositories>,
}

impl DocumentVisibility {
    #[must_use]
    pub const fn new(access: Arc<DocumentAccessService>, repos: Arc<DocumentRepositories>) -> Self {
        Self { access, repos }
    }

    async fn document(
        &self,
        document: &DocumentId,
        reader: &UserId,
    ) -> Result<Visibility, VisibilityError> {
        let found = self
            .repos
            .documents
            .find(document)
            .await
            .map_err(VisibilityError::from)?;
        let Some(document) = found else {
            return Ok(Visibility::Hidden);
        };
        let level = self
            .access
            .level_of(&document, reader)
            .await
            .map_err(|error| failed(DOCUMENT, error))?;
        Ok(verdict(level))
    }

    async fn documents(
        &self,
        document: &DocumentId,
        readers: &[UserId],
    ) -> Result<Vec<Visibility>, VisibilityError> {
        let found = self
            .repos
            .documents
            .find(document)
            .await
            .map_err(VisibilityError::from)?;
        let Some(document) = found else {
            return Ok(vec![Visibility::Hidden; readers.len()]);
        };
        let levels = self
            .access
            .levels_of(&document, readers)
            .await
            .map_err(|error| failed(DOCUMENT, error))?;
        Ok(levels.into_iter().map(verdict).collect())
    }

    async fn live_document_of(&self, comment: &str) -> Result<Option<DocumentId>, VisibilityError> {
        self.repos
            .comments
            .find_live_document_of(&DocumentCommentId::new(comment))
            .await
            .map_err(VisibilityError::from)
    }
}

fn verdict(level: Option<AccessLevel>) -> Visibility {
    level.map_or(Visibility::Hidden, |level| Visibility::Visible {
        excerpt_allowed: level >= AccessLevel::Comment,
    })
}

fn failed(kind: &str, error: DocumentsError) -> VisibilityError {
    VisibilityError::Other {
        kind: kind.to_owned(),
        source: Box::new(error),
    }
}

#[async_trait]
impl ResourceVisibility for DocumentVisibility {
    fn kinds(&self) -> &'static [&'static str] {
        &[DOCUMENT, FOLDER, COMMENT]
    }

    async fn visible(
        &self,
        subject: &SubjectRef,
        reader: &UserId,
    ) -> Result<Visibility, VisibilityError> {
        match subject.kind.as_str() {
            DOCUMENT => {
                self.document(&DocumentId::new(subject.id.as_str()), reader)
                    .await
            },
            COMMENT => match self.live_document_of(subject.id.as_str()).await? {
                Some(document) => self.document(&document, reader).await,
                None => Ok(Visibility::Hidden),
            },
            _ => Ok(Visibility::Hidden),
        }
    }

    async fn visible_many(
        &self,
        subject: &SubjectRef,
        readers: &[UserId],
    ) -> Result<Vec<Visibility>, VisibilityError> {
        match subject.kind.as_str() {
            DOCUMENT => {
                self.documents(&DocumentId::new(subject.id.as_str()), readers)
                    .await
            },
            COMMENT => match self.live_document_of(subject.id.as_str()).await? {
                Some(document) => self.documents(&document, readers).await,
                None => Ok(vec![Visibility::Hidden; readers.len()]),
            },
            _ => Ok(vec![Visibility::Hidden; readers.len()]),
        }
    }
}

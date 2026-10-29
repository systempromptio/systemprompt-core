//! The document lifecycle after creation: trash and restore, copy, and the
//! one metadata update (rename and move).
//!
//! A trashed document is hidden from listings, reads and writes until it is
//! restored, and trashing rejects its open proposals; `DocumentTrashPurgeJob`
//! hard-deletes it, with its revisions and node ids, after
//! `trash_retention_days`. Restoring into a folder that no longer exists lands
//! the document at the root, and a name taken in the meantime gains the first
//! free ` (restored)`, ` (restored 2)`, … suffix; a copy without a name takes
//! the first free ` (copy)`, ` (copy 2)`, … name in its folder. Both retry once
//! when a concurrent write takes the name. A copy starts a new revision chain
//! from the source's head bytes and belongs to the caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;
use systemprompt_traits::{ConstraintKind, RepositoryError};

use super::documents::NewContent;
use super::support::{document_name, join_path};
use super::{Author, DocumentService};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};

const COPY: &str = "copy";
const RESTORED: &str = "restored";
const MAX_SUFFIX: u32 = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyRequest {
    pub source: DocumentId,
    pub name: Option<String>,
    pub folder: Option<DocumentFolderId>,
    pub author: Author,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateRequest {
    pub name: Option<String>,
    pub folder: FolderMove,
}

/// Where an update leaves a document: in its folder, at the root, or in
/// another folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FolderMove {
    #[default]
    Keep,
    Root,
    Into(DocumentFolderId),
}

impl FolderMove {
    #[must_use]
    pub fn from_present(folder: Option<DocumentFolderId>) -> Self {
        folder.map_or(Self::Root, Self::Into)
    }

    #[must_use]
    pub const fn is_keep(&self) -> bool {
        matches!(self, Self::Keep)
    }

    #[must_use]
    pub const fn target(&self) -> Option<&DocumentFolderId> {
        match self {
            Self::Keep | Self::Root => None,
            Self::Into(id) => Some(id),
        }
    }
}

const TRASHED_NOTE: &str = "the document was moved to the trash";

impl DocumentService {
    pub async fn trash(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        let trashed = self
            .authorized_document(who, document, DocumentAction::Delete, trace)
            .await?;
        self.reject_open_for_document(&trashed, Some(TRASHED_NOTE.to_owned()))
            .await?;
        self.close_live_state(who, &trashed, trace).await?;
        let from = self.document_path(&trashed).await?;
        if !self.repos.documents.trash(document, &who.id, &from).await? {
            return Err(DocumentsError::not_found("document", document));
        }
        Ok(())
    }

    #[cfg(feature = "collab")]
    async fn close_live_state(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        if let Some(collab) = self.collab() {
            collab
                .close_for_trash(who, document, trace)
                .await
                .map_err(|source| DocumentsError::LiveSession(Box::new(source)))?;
        }
        self.release_live_state(document).await
    }

    #[cfg(not(feature = "collab"))]
    async fn close_live_state(
        &self,
        _who: &AuthenticatedUser,
        document: &Document,
        _trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.release_live_state(document).await
    }

    async fn release_live_state(&self, document: &Document) -> Result<(), DocumentsError> {
        self.repos.locks.release_all(&document.id).await?;
        self.repos.presence.clear(&document.id).await?;
        Ok(())
    }

    pub async fn untrash(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        let trashed = self
            .access
            .authorize(
                who,
                AccessTarget::Trashed(document.clone()),
                DocumentAction::Delete,
                trace,
            )
            .await?
            .document
            .ok_or_else(|| DocumentsError::not_found("trashed document", document))?;
        let folder = match &trashed.folder_id {
            Some(folder) if self.repos.folders.find(folder).await?.is_none() => {
                self.repos.documents.move_to_any(document, None).await?;
                None
            },
            other => other.clone(),
        };
        let mut attempt = 0;
        loop {
            attempt += 1;
            let current = self.load_trashed(document).await?;
            let name = self
                .free_name(folder.as_ref(), &trashed.name, RESTORED)
                .await?;
            if name != current.name {
                self.repos.documents.rename_any(document, &name).await?;
            }
            let to = join_path(self.folder_path(folder.as_ref()).await?.as_deref(), &name);
            match self.repos.documents.restore(document, &to).await {
                Ok(true) => return self.load_document(document).await,
                Ok(false) => return Err(DocumentsError::not_found("trashed document", document)),
                Err(error) if attempt == 1 && is_name_taken(&error) => {},
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub async fn copy(
        &self,
        who: &AuthenticatedUser,
        request: CopyRequest,
        trace: &TraceId,
    ) -> Result<(Document, DocumentRevision), DocumentsError> {
        let source = self
            .authorized_document(who, &request.source, DocumentAction::View, trace)
            .await?;
        let bytes = self
            .revision_bytes(&self.resolve_revision(&source, None).await?)
            .await?;
        let carried = self.access.check_copy(who, &source).await?;
        let folder = request.folder.or_else(|| source.folder_id.clone());
        self.authorize_create(who, folder.as_ref(), trace).await?;
        let explicit = request
            .name
            .map(|name| document_name(&name, source.format))
            .transpose()?;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let name = match &explicit {
                Some(name) => name.clone(),
                None => self.free_name(folder.as_ref(), &source.name, COPY).await?,
            };
            let content = NewContent {
                folder: folder.clone(),
                name,
                format: source.format,
                template_id: source.template_id.clone(),
                bytes: bytes.clone(),
                author: request.author.clone(),
                message: Some(format!("copy of {}", source.id)),
                ops: None,
                mime_type: Some(source.mime_type.clone()),
            };
            match self.create_with_bytes(who, content).await {
                Err(DocumentsError::Repository(error))
                    if attempt == 1 && explicit.is_none() && is_name_taken(&error) => {},
                Ok(copied) => return self.carry_cap(who, &source, copied, carried).await,
                other => return other,
            }
        }
    }

    async fn load_trashed(&self, document: &DocumentId) -> Result<Document, DocumentsError> {
        self.repos
            .documents
            .find_including_trashed(document)
            .await?
            .ok_or_else(|| DocumentsError::not_found("trashed document", document))
    }

    async fn free_name(
        &self,
        folder: Option<&DocumentFolderId>,
        name: &str,
        label: &str,
    ) -> Result<String, DocumentsError> {
        if label == RESTORED
            && self
                .repos
                .documents
                .find_in_folder(folder, name)
                .await?
                .is_none()
        {
            return Ok(name.to_owned());
        }
        let (stem, extension) = split_extension(name);
        for n in 1..=MAX_SUFFIX {
            let suffix = if n == 1 {
                format!(" ({label})")
            } else {
                format!(" ({label} {n})")
            };
            let candidate = format!("{stem}{suffix}{extension}");
            if self
                .repos
                .documents
                .find_in_folder(folder, &candidate)
                .await?
                .is_none()
            {
                return Ok(candidate);
            }
        }
        Err(DocumentsError::InvalidName {
            name: name.to_owned(),
        })
    }

    pub async fn update(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        request: UpdateRequest,
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        if let Some(name) = &request.name {
            self.rename(who, document, name, trace).await?;
        }
        if !request.folder.is_keep() {
            self.move_to(who, document, request.folder.target(), trace)
                .await?;
        }
        self.load_document(document).await
    }
}

fn split_extension(name: &str) -> (&str, &str) {
    name.rfind('.')
        .filter(|&dot| dot > 0)
        .map_or((name, ""), |dot| name.split_at(dot))
}

const fn is_name_taken(error: &RepositoryError) -> bool {
    matches!(
        error,
        RepositoryError::Constraint {
            kind: ConstraintKind::Unique,
            ..
        }
    )
}

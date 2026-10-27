//! The document lifecycle after creation: trash and restore, copy, and the
//! one metadata update (rename and move).
//!
//! A trashed document is hidden from listings, reads and writes until it is
//! restored; `DocumentTrashPurgeJob` hard-deletes it, with its revisions and
//! node ids, after `trash_retention_days`. Restoring into a folder that
//! no longer exists lands the document at the root, and a name taken in the
//! meantime gains a ` (restored)` suffix. A copy starts a new revision chain
//! from the source's head bytes and belongs to the caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::documents::NewContent;
use super::support::document_name;
use super::{Author, DocumentService};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};

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
    pub folder: Option<Option<DocumentFolderId>>,
}

impl DocumentService {
    pub async fn trash(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.authorized_document(who, document, DocumentAction::Delete, trace)
            .await?;
        if !self.repos.documents.trash(document, &who.id).await? {
            return Err(DocumentsError::not_found("document", document));
        }
        Ok(())
    }

    pub async fn restore_document(
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
        if self
            .repos
            .documents
            .find_in_folder(folder.as_ref(), &trashed.name)
            .await?
            .is_some()
        {
            let name = restored_name(&trashed.name);
            self.repos.documents.rename_any(document, &name).await?;
        }
        if !self.repos.documents.restore(document).await? {
            return Err(DocumentsError::not_found("trashed document", document));
        }
        self.load_document(document).await
    }

    pub async fn copy(
        &self,
        who: &AuthenticatedUser,
        request: CopyRequest,
        trace: &TraceId,
    ) -> Result<(Document, DocumentRevision), DocumentsError> {
        let (source, bytes) = {
            let source = self
                .authorized_document(who, &request.source, DocumentAction::View, trace)
                .await?;
            let head = self.resolve_revision(&source, None).await?;
            let bytes = self.revision_bytes(&head).await?;
            (source, bytes)
        };
        let folder = request.folder.or_else(|| source.folder_id.clone());
        self.authorize_create(who, folder.as_ref(), trace).await?;
        let name = match request.name {
            Some(name) => document_name(&name, source.format)?,
            None => copy_name(&source.name),
        };
        self.create_with_bytes(
            who,
            NewContent {
                folder,
                name,
                format: source.format,
                template_id: source.template_id.clone(),
                bytes,
                author: request.author,
                message: Some(format!("copy of {}", source.id)),
                ops: None,
            },
        )
        .await
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
        if let Some(folder) = &request.folder {
            self.move_to(who, document, folder.as_ref(), trace).await?;
        }
        self.load_document(document).await
    }
}

fn split_extension(name: &str) -> (&str, &str) {
    name.rfind('.')
        .filter(|&dot| dot > 0)
        .map_or((name, ""), |dot| name.split_at(dot))
}

fn restored_name(name: &str) -> String {
    let (stem, extension) = split_extension(name);
    format!("{stem} (restored){extension}")
}

fn copy_name(name: &str) -> String {
    let (stem, extension) = split_extension(name);
    format!("{stem} (copy){extension}")
}

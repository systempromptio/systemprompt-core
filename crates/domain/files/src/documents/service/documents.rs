//! Document creation, import and metadata changes.
//!
//! A document name always ends in its format's extension (one is appended
//! when missing) so that a Drive path maps to a local file the right
//! application opens.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{DocumentFolderId, DocumentId, DocumentTemplateId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentFormat};

use super::support::{blocking, document_name, format_of_name};
use super::{Author, CreateRequest, DocumentService, DocumentView, ImportRequest};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, NewDocument, NewRevision};

struct NewContent {
    folder: Option<DocumentFolderId>,
    name: String,
    format: DocumentFormat,
    template_id: Option<DocumentTemplateId>,
    bytes: Vec<u8>,
    author: Author,
    message: Option<String>,
}

impl DocumentService {
    pub async fn create_from_template(
        &self,
        who: &AuthenticatedUser,
        request: CreateRequest,
        trace: &TraceId,
    ) -> Result<(Document, DocumentRevision), DocumentsError> {
        self.authorize_create(who, request.folder.as_ref(), trace)
            .await?;
        let templates = Arc::clone(&self.templates);
        let codecs = Arc::clone(&self.codecs);
        let template_id = request.template_id.clone();
        let params = request.params;
        let instantiated = blocking(move || {
            templates
                .instantiate(&template_id, &params, &codecs)
                .map_err(|source| DocumentsError::Template {
                    template_id,
                    source,
                })
        })
        .await?;
        self.create_with_bytes(
            who,
            NewContent {
                folder: request.folder,
                name: document_name(&request.name, instantiated.format)?,
                format: instantiated.format,
                template_id: Some(instantiated.template_id),
                bytes: instantiated.bytes,
                author: request.author,
                message: request.message,
            },
        )
        .await
    }

    pub async fn import(
        &self,
        who: &AuthenticatedUser,
        request: ImportRequest,
        trace: &TraceId,
    ) -> Result<(Document, DocumentRevision), DocumentsError> {
        self.check_size(request.bytes.len())?;
        let format = request
            .format
            .or_else(|| format_of_name(&request.name))
            .ok_or_else(|| DocumentsError::UnknownFormat {
                name: request.name.clone(),
            })?;
        let name = document_name(&request.name, format)?;
        self.authorize_create(who, request.folder.as_ref(), trace)
            .await?;
        let codec = self.codec(format)?;
        let bytes = blocking(move || {
            codec.project(&request.bytes)?;
            Ok(request.bytes)
        })
        .await?;
        self.create_with_bytes(
            who,
            NewContent {
                folder: request.folder,
                name,
                format,
                template_id: None,
                bytes,
                author: request.author,
                message: None,
            },
        )
        .await
    }

    async fn authorize_create(
        &self,
        who: &AuthenticatedUser,
        folder: Option<&DocumentFolderId>,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.access
            .authorize(
                who,
                AccessTarget::Folder(folder.cloned()),
                DocumentAction::Create,
                trace,
            )
            .await
            .map(|_| ())
    }

    async fn create_with_bytes(
        &self,
        who: &AuthenticatedUser,
        content: NewContent,
    ) -> Result<(Document, DocumentRevision), DocumentsError> {
        self.check_size(content.bytes.len())?;
        let blob = self.blobs.put(&content.bytes).await?;
        let document = self
            .repos
            .documents
            .insert(NewDocument {
                folder_id: content.folder.clone(),
                name: content.name.clone(),
                format: content.format,
                mime_type: content.format.mime_type().to_owned(),
                template_id: content.template_id.clone(),
                owner_user_id: who.id.clone(),
                created_by: who.id.clone(),
            })
            .await?;
        let revision = match self
            .first_revision(who, &document, blob.hash, &content)
            .await
        {
            Ok(revision) => revision,
            Err(error) => {
                if let Err(undo) = self.repos.documents.soft_delete(&document.id).await {
                    tracing::error!(
                        document_id = %document.id,
                        error = %undo,
                        "A document whose first revision failed could not be withdrawn"
                    );
                }
                return Err(error);
            },
        };
        self.index_text(&document, &revision, content.bytes).await;
        self.announce_revision(&document, &revision).await;
        let document = self.load_document(&document.id).await?;
        Ok((document, revision))
    }

    async fn first_revision(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        hash: systemprompt_identifiers::ContentHash,
        content: &NewContent,
    ) -> Result<DocumentRevision, DocumentsError> {
        let author = &content.author;
        let revision = self
            .repos
            .revisions
            .insert(NewRevision {
                document_id: document.id.clone(),
                parent_revision_id: None,
                content_sha256: hash,
                author_user_id: who.id.clone(),
                actor_kind: author.actor_kind,
                origin_kind: author.origin.kind(),
                origin_id: author.origin.id(),
                message: content.message.clone(),
                edit_ops: None,
            })
            .await?;
        if !self
            .repos
            .documents
            .set_head_if(&document.id, None, &revision.id)
            .await?
        {
            let head = self.repos.revisions.head(&document.id).await?;
            return Err(DocumentsError::Conflict {
                document_id: document.id.clone(),
                head: head.map_or_else(|| revision.id.clone(), |head| head.id),
                base: None,
            });
        }
        Ok(revision)
    }

    pub async fn get(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<DocumentView, DocumentsError> {
        let granted = self
            .access
            .authorize(
                who,
                AccessTarget::Document(document.clone()),
                DocumentAction::View,
                trace,
            )
            .await?;
        let document = granted
            .document
            .ok_or_else(|| DocumentsError::not_found("document", document))?;
        let head = self.repos.revisions.head(&document.id).await?;
        let folder_path = self.folder_path(document.folder_id.as_ref()).await?;
        Ok(DocumentView {
            document,
            head,
            my_access: granted.level,
            folder_path,
        })
    }

    pub async fn rename(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        name: &str,
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        let current = self
            .authorized_document(who, document, DocumentAction::Edit, trace)
            .await?;
        let name = document_name(name, current.format)?;
        if !self.repos.documents.rename(document, &name).await? {
            return Err(DocumentsError::not_found("document", document));
        }
        self.load_document(document).await
    }

    pub async fn move_to(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        folder: Option<&DocumentFolderId>,
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        self.authorized_document(who, document, DocumentAction::Own, trace)
            .await?;
        self.authorize_create(who, folder, trace).await?;
        if !self.repos.documents.move_to(document, folder).await? {
            return Err(DocumentsError::not_found("document", document));
        }
        self.load_document(document).await
    }

    pub async fn delete(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.authorized_document(who, document, DocumentAction::Delete, trace)
            .await?;
        if !self.repos.documents.soft_delete(document).await? {
            return Err(DocumentsError::not_found("document", document));
        }
        Ok(())
    }
}

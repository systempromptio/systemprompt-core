//! Reading documents: revisions, bytes, projections, previews and search.
//!
//! A projection is returned id-addressed: every node carries its stable id,
//! with the codec path in its `path` attribute.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, Projection};
use systemprompt_traits::StoredFileRead;

use super::support::blocking;
use super::{DocumentService, HistoryPage, SearchResult, StreamRequest};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};

impl DocumentService {
    pub(super) async fn authorized_document(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        action: DocumentAction,
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        self.access
            .authorize(who, AccessTarget::Document(document.clone()), action, trace)
            .await?
            .document
            .ok_or_else(|| DocumentsError::not_found("document", document))
    }

    pub(super) async fn load_document(
        &self,
        document: &DocumentId,
    ) -> Result<Document, DocumentsError> {
        self.repos
            .documents
            .find(document)
            .await?
            .filter(|d| d.trashed_at.is_none())
            .ok_or_else(|| DocumentsError::not_found("document", document))
    }

    pub(super) async fn resolve_revision(
        &self,
        document: &Document,
        revision: Option<&DocumentRevisionId>,
    ) -> Result<DocumentRevision, DocumentsError> {
        if let Some(id) = revision {
            return self
                .repos
                .revisions
                .find(id)
                .await?
                .filter(|r| r.document_id == document.id)
                .ok_or_else(|| DocumentsError::not_found("revision", id));
        }
        self.repos
            .revisions
            .head(&document.id)
            .await?
            .ok_or_else(|| DocumentsError::not_found("head revision of document", &document.id))
    }

    pub async fn history(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        page: HistoryPage,
        trace: &TraceId,
    ) -> Result<Vec<DocumentRevision>, DocumentsError> {
        self.authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        Ok(self
            .repos
            .revisions
            .list(document, page.limit, page.before)
            .await?)
    }

    pub async fn revision(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        revision: &DocumentRevisionId,
        trace: &TraceId,
    ) -> Result<DocumentRevision, DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        self.resolve_revision(&document, Some(revision)).await
    }

    pub async fn read_bytes(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        revision: Option<&DocumentRevisionId>,
        trace: &TraceId,
    ) -> Result<(DocumentRevision, Vec<u8>), DocumentsError> {
        let (_, revision, bytes) = self.read(who, document, revision, trace).await?;
        Ok((revision, bytes))
    }

    async fn read(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        revision: Option<&DocumentRevisionId>,
        trace: &TraceId,
    ) -> Result<(Document, DocumentRevision, Vec<u8>), DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        let revision = self.resolve_revision(&document, revision).await?;
        let bytes = self.revision_bytes(&revision).await?;
        Ok((document, revision, bytes))
    }

    pub async fn open_stream(
        &self,
        who: &AuthenticatedUser,
        request: StreamRequest<'_>,
        trace: &TraceId,
    ) -> Result<(DocumentRevision, StoredFileRead), DocumentsError> {
        let document = self
            .authorized_document(who, request.document, DocumentAction::View, trace)
            .await?;
        let revision = self.resolve_revision(&document, request.revision).await?;
        let read = self
            .blobs
            .open(&revision.content_sha256, request.range)
            .await?;
        Ok((revision, read))
    }

    pub async fn project(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        revision: Option<&DocumentRevisionId>,
        trace: &TraceId,
    ) -> Result<(DocumentRevision, Projection), DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        let revision = self.resolve_revision(&document, revision).await?;
        let projected = self.projected(&document, &revision).await?;
        Ok((revision, projected.ids))
    }

    pub async fn revision_size(
        &self,
        revision: &DocumentRevision,
    ) -> Result<Option<i64>, DocumentsError> {
        Ok(self
            .repos
            .blobs
            .find(&revision.content_sha256)
            .await?
            .map(|blob| blob.size_bytes))
    }

    pub async fn preview_html(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        revision: Option<&DocumentRevisionId>,
        trace: &TraceId,
    ) -> Result<(DocumentRevision, String), DocumentsError> {
        let (document, revision, bytes) = self.read(who, document, revision, trace).await?;
        let codec = self.codec(document.format)?;
        let html = blocking(move || Ok(codec.preview_html(&bytes)?)).await?;
        Ok((revision, html))
    }

    pub async fn plain_text(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        revision: Option<&DocumentRevisionId>,
        trace: &TraceId,
    ) -> Result<(DocumentRevision, String), DocumentsError> {
        let (document, revision, bytes) = self.read(who, document, revision, trace).await?;
        let text = self.plain_text_of(document.format, bytes).await?;
        Ok((revision, text))
    }

    pub async fn search(
        &self,
        who: &AuthenticatedUser,
        query: &str,
        limit: i64,
        trace: &TraceId,
    ) -> Result<Vec<SearchResult>, DocumentsError> {
        let pairs = self.access.subject_pairs(who).await?;
        let hits = self
            .repos
            .text_index
            .search(query, &pairs, &who.id, limit)
            .await?;
        let mut found = Vec::with_capacity(hits.len());
        for hit in hits {
            if let Some(document) = self.repos.documents.find(&hit.document_id).await?
                && document.trashed_at.is_none()
            {
                found.push((document, hit));
            }
        }
        let (documents, hits): (Vec<_>, Vec<_>) = found.into_iter().unzip();
        let visible = self.hook_visible(who, documents, trace).await;
        Ok(hits
            .into_iter()
            .filter_map(|hit| {
                visible
                    .iter()
                    .find(|d| d.id == hit.document_id)
                    .map(|document| SearchResult {
                        document: document.clone(),
                        hit,
                    })
            })
            .collect())
    }
}

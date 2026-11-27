//! Reading the Drive: listings, folders, a document's detail, its rendered and
//! preview pages, its bytes in each format the instance serves, its revisions
//! and its outline or text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Method;
use sp_protocol::ids::{DocumentFolderId, DocumentId, DocumentRevisionId};
use sp_protocol::wire::drive::{
    CreateFolderRequest, DriveDetail, DriveFolder, DriveListing, DriveOutline, DriveRevision,
    DriveRevisions, DriveScope,
};
use sp_protocol::wire::{headers, paths};

use super::types::{ContentFormat, DocumentBytes, ListQuery, ReadView, RenderedPage};
use super::{
    CONTENT_BODY, DriveClient, LIST_BODY, PAGE_BODY, READ_BODY, TRANSFER_TIMEOUT, revision_of,
    strip_empty_query,
};
use crate::error::DriveError;
use crate::http::Call;

const LIST: &str = "GET /api/v1/documents";
const FOLDERS: &str = "GET /api/v1/document-folders";
const CREATE_FOLDER: &str = "POST /api/v1/document-folders";
const DETAIL: &str = "GET /api/v1/documents/{id}";
const RENDERED: &str = "GET /api/v1/documents/{id}/rendered";
const PREVIEW: &str = "GET /api/v1/documents/{id}/preview";
const CONTENT: &str = "GET /api/v1/documents/{id}/content";
const EXPORT_MARKDOWN: &str = "GET /api/v1/documents/{id}/export/markdown";
const EXPORT_DOCX: &str = "GET /api/v1/documents/{id}/export/docx";
const REVISIONS: &str = "GET /api/v1/documents/{id}/revisions";
const READ: &str = "GET /api/v1/documents/{id}/read";

impl DriveClient {
    pub async fn list_documents(&self, query: &ListQuery) -> Result<DriveListing, DriveError> {
        let mut url = self.url(paths::DOCUMENTS, &[])?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("scope", query.scope.as_str());
            if let Some(folder) = &query.folder {
                pairs.append_pair("folder", folder.as_ref());
            }
            if let Some(text) = &query.query {
                pairs.append_pair("query", text);
            }
            if let Some(sort) = &query.sort {
                pairs.append_pair("sort", sort);
            }
            if let Some(limit) = query.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(cursor) = &query.cursor {
                pairs.append_pair("cursor", cursor);
            }
        }
        let call =
            Call::new(Method::GET, url, LIST).limits(crate::http::DEFAULT_TIMEOUT, LIST_BODY);
        self.json(call, &[200]).await
    }

    pub async fn search_documents(
        &self,
        text: &str,
        scope: DriveScope,
    ) -> Result<DriveListing, DriveError> {
        self.list_documents(&ListQuery {
            scope,
            query: Some(text.to_owned()),
            ..ListQuery::default()
        })
        .await
    }

    pub async fn folders(
        &self,
        parent: Option<&DocumentFolderId>,
    ) -> Result<Vec<DriveFolder>, DriveError> {
        let mut url = self.url(paths::DOCUMENT_FOLDERS, &[])?;
        if let Some(parent) = parent {
            url.query_pairs_mut().append_pair("folder", parent.as_ref());
        }
        let call =
            Call::new(Method::GET, url, FOLDERS).limits(crate::http::DEFAULT_TIMEOUT, LIST_BODY);
        let listing: DriveListing = self.json(call, &[200]).await?;
        Ok(listing.folders)
    }

    pub async fn create_folder(
        &self,
        request: &CreateFolderRequest,
    ) -> Result<DriveFolder, DriveError> {
        let call = Call::new(
            Method::POST,
            self.url(paths::DOCUMENT_FOLDERS, &[])?,
            CREATE_FOLDER,
        )
        .json(request)?;
        self.json(call, &[201]).await
    }

    pub async fn document(&self, id: &DocumentId) -> Result<DriveDetail, DriveError> {
        let call = Call::new(Method::GET, self.document_url(id, &[])?, DETAIL);
        self.json(call, &[200]).await
    }

    pub async fn rendered(
        &self,
        id: &DocumentId,
        revision: Option<&DocumentRevisionId>,
    ) -> Result<RenderedPage, DriveError> {
        self.page(id, "rendered", RENDERED, revision).await
    }

    pub async fn preview(
        &self,
        id: &DocumentId,
        revision: Option<&DocumentRevisionId>,
    ) -> Result<RenderedPage, DriveError> {
        self.page(id, "preview", PREVIEW, revision).await
    }

    pub async fn content(
        &self,
        id: &DocumentId,
        format: ContentFormat,
        revision: Option<&DocumentRevisionId>,
    ) -> Result<DocumentBytes, DriveError> {
        let (mut url, endpoint) = match format {
            ContentFormat::Stored | ContentFormat::Pdf => {
                (self.document_url(id, &["content"])?, CONTENT)
            },
            ContentFormat::Markdown => (
                self.document_url(id, &["export", "markdown"])?,
                EXPORT_MARKDOWN,
            ),
            ContentFormat::Docx => (self.document_url(id, &["export", "docx"])?, EXPORT_DOCX),
        };
        {
            let mut pairs = url.query_pairs_mut();
            if format == ContentFormat::Pdf {
                pairs.append_pair("format", "pdf");
            }
            if let Some(revision) = revision {
                pairs.append_pair("revision", revision.as_ref());
            }
        }
        strip_empty_query(&mut url);
        let call = Call::new(Method::GET, url, endpoint)
            .accept("*/*")
            .limits(TRANSFER_TIMEOUT, CONTENT_BODY);
        let answer = self.answer(call, &[200]).await?;
        Ok(DocumentBytes {
            content_type: answer.header("content-type"),
            revision: revision_of(&answer),
            sha256: answer.header(headers::DOCUMENT_CONTENT_SHA256),
            bytes: answer.body,
        })
    }

    pub async fn revisions(&self, id: &DocumentId) -> Result<Vec<DriveRevision>, DriveError> {
        let call = Call::new(
            Method::GET,
            self.document_url(id, &["revisions"])?,
            REVISIONS,
        )
        .limits(crate::http::DEFAULT_TIMEOUT, READ_BODY);
        let answer: DriveRevisions = self.json(call, &[200]).await?;
        Ok(answer.revisions)
    }

    pub async fn read(&self, id: &DocumentId, view: ReadView) -> Result<DriveOutline, DriveError> {
        let mut url = self.document_url(id, &["read"])?;
        url.query_pairs_mut().append_pair("view", view.as_str());
        let call =
            Call::new(Method::GET, url, READ).limits(crate::http::DEFAULT_TIMEOUT, READ_BODY);
        self.json(call, &[200]).await
    }

    async fn page(
        &self,
        id: &DocumentId,
        view: &'static str,
        endpoint: &'static str,
        revision: Option<&DocumentRevisionId>,
    ) -> Result<RenderedPage, DriveError> {
        let mut url = self.document_url(id, &[view])?;
        if let Some(revision) = revision {
            url.query_pairs_mut()
                .append_pair("revision", revision.as_ref());
        }
        let call = Call::new(Method::GET, url, endpoint)
            .accept("text/html")
            .limits(TRANSFER_TIMEOUT, PAGE_BODY);
        let answer = self.answer(call, &[200]).await?;
        Ok(RenderedPage {
            content_security_policy: answer.header("content-security-policy"),
            revision: revision_of(&answer),
            sha256: answer.header(headers::DOCUMENT_CONTENT_SHA256),
            html: String::from_utf8_lossy(&answer.body).into_owned(),
        })
    }
}

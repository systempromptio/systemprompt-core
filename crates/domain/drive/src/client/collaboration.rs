//! Working on documents: structured edits, proposals, access and agent caps,
//! sharing, comments, creating a document and naming the conversation a write
//! belongs to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Method;
use serde::Serialize;
use sp_protocol::ids::{ContextId, DocumentCommentId, DocumentFolderId, DocumentId};
use sp_protocol::wire::drive::{
    CommentRequest, CreateDocumentRequest, DriveAccess, DriveAgentCap, DriveComment, DriveComments,
    DriveContext, DriveCreated, DriveGrantMade, DriveLanded, DriveProposal, DriveProposals,
    DriveProposed, EditRequest, ShareRequest,
};
use sp_protocol::wire::paths;

use super::types::EditOutcome;
use super::{DriveClient, EmptyBody, READ_BODY, decode};
use crate::error::{ConflictHead, DriveError};
use crate::http::Call;

const EDITS: &str = "POST /api/v1/documents/{id}/edits";
const PROPOSALS: &str = "GET /api/v1/documents/{id}/proposals";
const ACCESS: &str = "GET /api/v1/documents/{id}/access";
const DOCUMENT_CAP: &str = "GET /api/v1/documents/{id}/agent-cap";
const FOLDER_CAP: &str = "GET /api/v1/document-folders/{id}/agent-cap";
const GRANTS: &str = "POST /api/v1/documents/{id}/grants";
const REVOKE: &str = "DELETE /api/v1/documents/{id}/grants";
const COMMENTS: &str = "GET /api/v1/documents/{id}/comments";
const COMMENT: &str = "POST /api/v1/documents/{id}/comments";
const RESOLVE: &str = "POST /api/v1/documents/{id}/comments/{comment}/resolve";
const CREATE: &str = "POST /api/v1/documents";
const CREATE_CONTEXT: &str = "POST /api/v1/core/contexts";
const RENAME_CONTEXT: &str = "PUT /api/v1/core/contexts/{id}";

#[derive(Serialize)]
struct ContextName<'a> {
    name: &'a str,
}

impl DriveClient {
    pub async fn edit(
        &self,
        id: &DocumentId,
        request: &EditRequest,
        context_id: Option<&ContextId>,
    ) -> Result<EditOutcome, DriveError> {
        let call = Call::new(Method::POST, self.document_url(id, &["edits"])?, EDITS)
            .json(request)?
            .context(context_id);
        let answer = self.answer(call, &[200, 201]).await?;
        if answer.status == 201 {
            return decode::<DriveProposed>(EDITS, &answer)
                .map(|proposed| EditOutcome::Proposed(Box::new(proposed)));
        }
        decode::<DriveLanded>(EDITS, &answer).map(|landed| EditOutcome::Landed(Box::new(landed)))
    }

    pub async fn edit_at_head(
        &self,
        id: &DocumentId,
        request: &EditRequest,
        context_id: Option<&ContextId>,
    ) -> Result<EditOutcome, DriveError> {
        if let Some(base) = &request.base_revision {
            let detail = self.document(id).await?;
            if &detail.document.head_revision_id != base {
                tracing::debug!(
                    endpoint = EDITS,
                    "edit refused locally: the document moved past its base"
                );
                return Err(DriveError::Conflict {
                    endpoint: EDITS,
                    error_key: Some("stale_base".to_owned()),
                    head: ConflictHead {
                        head_revision_id: Some(detail.document.head_revision_id),
                        head_content_sha256: Some(detail.head.content_sha256),
                        base_revision_id: Some(base.clone()),
                    },
                });
            }
        }
        self.edit(id, request, context_id).await
    }

    pub async fn proposals(&self, id: &DocumentId) -> Result<Vec<DriveProposal>, DriveError> {
        let call = Call::new(
            Method::GET,
            self.document_url(id, &["proposals"])?,
            PROPOSALS,
        )
        .limits(crate::http::DEFAULT_TIMEOUT, READ_BODY);
        let answer: DriveProposals = self.json(call, &[200]).await?;
        Ok(answer.proposals)
    }

    pub async fn access(&self, id: &DocumentId) -> Result<DriveAccess, DriveError> {
        let call = Call::new(Method::GET, self.document_url(id, &["access"])?, ACCESS);
        self.json(call, &[200]).await
    }

    pub async fn document_agent_cap(&self, id: &DocumentId) -> Result<DriveAgentCap, DriveError> {
        let call = Call::new(
            Method::GET,
            self.document_url(id, &["agent-cap"])?,
            DOCUMENT_CAP,
        );
        self.json(call, &[200]).await
    }

    pub async fn folder_agent_cap(
        &self,
        id: &DocumentFolderId,
    ) -> Result<DriveAgentCap, DriveError> {
        let folder = id.to_string();
        let call = Call::new(
            Method::GET,
            self.url(paths::DOCUMENT_FOLDERS, &[folder.as_str(), "agent-cap"])?,
            FOLDER_CAP,
        );
        self.json(call, &[200]).await
    }

    pub async fn share(
        &self,
        id: &DocumentId,
        request: &ShareRequest,
    ) -> Result<DriveGrantMade, DriveError> {
        let call =
            Call::new(Method::POST, self.document_url(id, &["grants"])?, GRANTS).json(request)?;
        self.json(call, &[201]).await
    }

    pub async fn revoke(&self, id: &DocumentId, grant_id: &str) -> Result<(), DriveError> {
        let mut url = self.document_url(id, &["grants"])?;
        url.query_pairs_mut().append_pair("grant_id", grant_id);
        self.answer(Call::new(Method::DELETE, url, REVOKE), &[200, 204])
            .await
            .map(|_| ())
    }

    pub async fn comments(&self, id: &DocumentId) -> Result<Vec<DriveComment>, DriveError> {
        let call = Call::new(Method::GET, self.document_url(id, &["comments"])?, COMMENTS)
            .limits(crate::http::DEFAULT_TIMEOUT, READ_BODY);
        let answer: DriveComments = self.json(call, &[200]).await?;
        Ok(answer.comments)
    }

    pub async fn create_comment(
        &self,
        id: &DocumentId,
        request: &CommentRequest,
        context_id: Option<&ContextId>,
    ) -> Result<DriveComment, DriveError> {
        if request.anchor.is_none() && request.parent_id.is_none() {
            return Err(DriveError::InvalidInput {
                what: "a comment needs an anchor or a parent",
            });
        }
        let call = Call::new(Method::POST, self.document_url(id, &["comments"])?, COMMENT)
            .json(request)?
            .context(context_id);
        self.json(call, &[201]).await
    }

    pub async fn resolve_comment(
        &self,
        id: &DocumentId,
        comment: &DocumentCommentId,
    ) -> Result<DriveComment, DriveError> {
        let comment = comment.to_string();
        let url = self.document_url(id, &["comments", comment.as_str(), "resolve"])?;
        let call = Call::new(Method::POST, url, RESOLVE).json(&EmptyBody {})?;
        self.json(call, &[200]).await
    }

    pub async fn create_document(
        &self,
        request: &CreateDocumentRequest,
        context_id: Option<&ContextId>,
    ) -> Result<DriveCreated, DriveError> {
        let call = Call::new(Method::POST, self.url(paths::DOCUMENTS, &[])?, CREATE)
            .json(request)?
            .context(context_id);
        self.json(call, &[201]).await
    }

    pub async fn create_context(&self, name: &str) -> Result<ContextId, DriveError> {
        let call = Call::new(
            Method::POST,
            self.url(paths::CONTEXTS, &[])?,
            CREATE_CONTEXT,
        )
        .json(&ContextName { name })?;
        let context: DriveContext = self.json(call, &[201]).await?;
        Ok(context.data.context_id)
    }

    pub async fn rename_context(
        &self,
        context_id: &ContextId,
        name: &str,
    ) -> Result<(), DriveError> {
        let url = self.url(paths::CONTEXTS, &[context_id.as_str()])?;
        let call = Call::new(Method::PUT, url, RENAME_CONTEXT).json(&ContextName { name })?;
        self.answer(call, &[200]).await.map(|_| ())
    }
}

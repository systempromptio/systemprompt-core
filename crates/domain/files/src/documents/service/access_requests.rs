//! Requests for access from a signed-in user who holds no level on a
//! document, and the owner's answer.
//!
//! Requests are off unless `files.documents.accessRequests` is set; with
//! them off a request is refused as `not_found`, exactly like any document
//! the caller cannot see. A user holds at most one open request per
//! document. A request reaches the document's owner as
//! `document.access_requested`. Answering needs `share`. Granting first runs
//! every sharing rule [`DocumentService::share`] applies, then records the
//! answer as a compare-and-set from `open` and writes the grant and its audit
//! row in one transaction, so of two concurrent answers exactly one lands and
//! a request is never granted without its grant. Declining only closes it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentAccessRequestId, DocumentId, TraceId};
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::documents::{
    AccessLevel, AccessRequestStatus, DocumentAccessRequestedEvent, DocumentAction,
};
use systemprompt_traits::RepositoryError;

use super::DocumentService;
use super::grants::{ShareRequest, check_message};
use super::ownership::USER_SUBJECT;
use crate::documents::error::DocumentsError;
use crate::documents::models::{DocumentAccessRequest, GrantTarget, NewAccessRequest};
use crate::documents::repository::DocumentAccessRequestRepository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessRequestInput {
    pub level: AccessLevel,
    pub message: Option<String>,
}

/// One access request on one document and the answer to give it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecideRequest {
    pub document: DocumentId,
    pub request: DocumentAccessRequestId,
    pub decision: AccessDecision,
}

/// The owner's answer to an access request. `subject_display` is the name
/// the resulting grant shows for the requester.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessDecision {
    Grant { subject_display: Option<String> },
    Decline,
}

fn signed_in(who: &AuthenticatedUser) -> bool {
    !who.permissions.contains(&Permission::Anonymous)
}

impl DocumentService {
    pub async fn can_request_access(&self, who: &AuthenticatedUser, document: &DocumentId) -> bool {
        if !self.config.access_requests || !signed_in(who) {
            return false;
        }
        match self.repos.documents.find(document).await {
            Ok(Some(found)) => matches!(self.access.level_of(&found, &who.id).await, Ok(None)),
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(document_id = %document, error = %error, "Access request eligibility was not read");
                false
            },
        }
    }

    pub async fn request_access(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        input: AccessRequestInput,
    ) -> Result<DocumentAccessRequest, DocumentsError> {
        if !self.config.access_requests || !signed_in(who) {
            return Err(DocumentsError::not_found("document", document));
        }
        if input.level == AccessLevel::Own {
            return Err(DocumentsError::OwnNotGrantable);
        }
        check_message(input.message.as_deref())?;
        let found = self
            .repos
            .documents
            .find(document)
            .await?
            .ok_or_else(|| DocumentsError::not_found("document", document))?;
        if let Some(held) = self.access.level_of(&found, &who.id).await? {
            return Err(DocumentsError::AccessRequestNotNeeded {
                document_id: document.clone(),
                held,
            });
        }
        let request = self
            .repos
            .access_requests
            .create(&NewAccessRequest {
                document_id: document.clone(),
                requester_user_id: who.id.clone(),
                level: input.level,
                message: input.message,
            })
            .await?
            .ok_or_else(|| DocumentsError::AccessRequestOpen {
                document_id: document.clone(),
            })?;
        let event = DocumentAccessRequestedEvent {
            document_id: document.clone(),
            document_name: found.name.clone(),
            request_id: request.id.clone(),
            requester_user_id: who.id.clone(),
            requester_display: who.username.clone(),
            level: request.level,
            message: request.message.clone(),
        };
        if let Err(error) = self
            .events
            .access_requested(std::slice::from_ref(&found.owner_user_id), &event)
            .await
        {
            tracing::warn!(document_id = %document, error = %error, "Access request event not relayed");
        }
        Ok(request)
    }

    pub async fn access_requests(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<Vec<DocumentAccessRequest>, DocumentsError> {
        self.authorized_document(who, document, DocumentAction::Share, trace)
            .await?;
        Ok(self.repos.access_requests.list_open(document).await?)
    }

    pub async fn decide_access_request(
        &self,
        who: &AuthenticatedUser,
        decide: DecideRequest,
        trace: &TraceId,
    ) -> Result<DocumentAccessRequest, DocumentsError> {
        let DecideRequest {
            document,
            request,
            decision,
        } = decide;
        let (document, request) = (&document, &request);
        self.authorized_document(who, document, DocumentAction::Share, trace)
            .await?;
        let open = self.open_request(document, request).await?;
        let decided = match decision {
            AccessDecision::Grant { subject_display } => {
                let share = ShareRequest {
                    target: GrantTarget::Document(document.clone()),
                    subject_kind: USER_SUBJECT.to_owned(),
                    subject_value: open.requester_user_id.as_str().to_owned(),
                    subject_display,
                    level: open.level,
                    valid_until: None,
                    message: None,
                    via_agent: None,
                };
                self.grant_request(who, request, share, trace).await?
            },
            AccessDecision::Decline => {
                self.repos
                    .access_requests
                    .decide(request, AccessRequestStatus::Declined, &who.id)
                    .await?
            },
        };
        if !decided {
            let current = self.load_request(document, request).await?;
            return Err(DocumentsError::AccessRequestClosed {
                request_id: request.clone(),
                status: current.status,
            });
        }
        self.load_request(document, request).await
    }

    async fn grant_request(
        &self,
        who: &AuthenticatedUser,
        request: &DocumentAccessRequestId,
        share: ShareRequest,
        trace: &TraceId,
    ) -> Result<bool, DocumentsError> {
        let checked = self.check_share(who, share, trace).await?;
        let mut tx = self.repos.access_requests.begin().await?;
        if !DocumentAccessRequestRepository::decide_in(
            &mut tx,
            request,
            AccessRequestStatus::Granted,
            &who.id,
        )
        .await?
        {
            tx.rollback().await.map_err(sql)?;
            return Ok(false);
        }
        let grant = self
            .repos
            .grants
            .insert(&mut tx, checked.grant.clone())
            .await?;
        self.record_checked(&mut tx, who, &checked, &grant).await?;
        tx.commit().await.map_err(sql)?;
        self.announce_checked(who, checked, &grant).await;
        Ok(true)
    }

    async fn open_request(
        &self,
        document: &DocumentId,
        request: &DocumentAccessRequestId,
    ) -> Result<DocumentAccessRequest, DocumentsError> {
        let found = self.load_request(document, request).await?;
        if found.status == AccessRequestStatus::Open {
            Ok(found)
        } else {
            Err(DocumentsError::AccessRequestClosed {
                request_id: request.clone(),
                status: found.status,
            })
        }
    }

    async fn load_request(
        &self,
        document: &DocumentId,
        request: &DocumentAccessRequestId,
    ) -> Result<DocumentAccessRequest, DocumentsError> {
        self.repos
            .access_requests
            .find(document, request)
            .await?
            .ok_or_else(|| DocumentsError::not_found("access request", request))
    }
}

fn sql(error: sqlx::Error) -> DocumentsError {
    DocumentsError::Repository(RepositoryError::from(error))
}

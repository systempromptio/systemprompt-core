//! Ownership transfer, revoking every grant at once, and the
//! `document.shared` event.
//!
//! Transferring ownership and revoking every grant are the owner's alone;
//! the previous owner keeps an `edit` grant. The recipient of a transfer is
//! validated by the caller layer, which resolves it to an active user and
//! hands its display name in [`TransferRequest`]. The owner change, the
//! previous owner's `edit` grant and their audit rows are one statement.
//!
//! `document.shared` is addressed per recipient: a `user` subject is the
//! user, a role or dimension subject is resolved to its members at emit time
//! (capped, with the truncation logged). The sharer is never a recipient.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentCommentId, DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentOwnershipTransferredEvent, DocumentSharedEvent, SHARED_EVENT_VERSION,
};

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentGrant, GrantActor, OwnerTransfer};

pub(super) const USER_SUBJECT: &str = "user";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ShareCause {
    pub(super) via_agent: Option<String>,
    pub(super) comment_id: Option<DocumentCommentId>,
}

/// The new owner of a document. `to_display` is `None` when the caller
/// layer could not resolve `to` to an active user; the transfer is then
/// refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferRequest {
    pub to: UserId,
    pub to_display: Option<String>,
}

impl DocumentService {
    pub async fn transfer_ownership(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        request: TransferRequest,
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        let current = self
            .authorized_document(who, document, DocumentAction::Own, trace)
            .await?;
        let TransferRequest { to, to_display } = request;
        if to_display.is_none() {
            return Err(DocumentsError::UnknownSubject {
                kind: USER_SUBJECT.to_owned(),
                value: to.as_str().to_owned(),
            });
        }
        let transfer = OwnerTransfer {
            document_id: document.clone(),
            from: current.owner_user_id.clone(),
            from_display: who.username.clone(),
            to: to.clone(),
            actor: GrantActor {
                user: who.id.clone(),
                trace: trace.clone(),
            },
        };
        if !self.repos.grant_events.transfer_owner(&transfer).await? {
            return Err(DocumentsError::not_found("document", document));
        }
        let event = DocumentOwnershipTransferredEvent {
            document_id: document.clone(),
            from: current.owner_user_id.clone(),
            to: to.clone(),
        };
        let transferred = self.load_document(document).await?;
        let mut recipients = self.recipients(&transferred).await;
        if !recipients.contains(&current.owner_user_id) {
            recipients.push(current.owner_user_id.clone());
        }
        if let Err(error) = self.events.ownership_transferred(&recipients, &event).await {
            tracing::warn!(document_id = %document, error = %error, "Ownership transfer event not relayed");
        }
        Ok(transferred)
    }

    pub async fn revoke_all(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<u64, DocumentsError> {
        let current = self
            .authorized_document(who, document, DocumentAction::Own, trace)
            .await?;
        let grants = self.repos.grants.list_for_document(document).await?;
        let actor = GrantActor {
            user: who.id.clone(),
            trace: trace.clone(),
        };
        let revoked = self
            .repos
            .grants
            .delete_for_document(document, &actor)
            .await?;
        for grant in &grants {
            self.clear_presence(grant, Some(&current.owner_user_id))
                .await;
        }
        Ok(revoked)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the share notice names the actor, the document, the grant, the message and the cause, each distinct"
    )]
    pub(super) async fn announce_share(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        grant: &DocumentGrant,
        message: Option<String>,
        cause: &ShareCause,
    ) {
        let recipients = if grant.subject_kind == USER_SUBJECT {
            vec![UserId::new(grant.subject_value.clone())]
        } else {
            match self
                .access
                .subject_members(&grant.subject_kind, &grant.subject_value)
                .await
            {
                Ok(members) => members,
                Err(error) => {
                    tracing::warn!(document_id = %document.id, error = %error, "Share recipients were not resolved; no share event sent");
                    return;
                },
            }
        };
        for recipient in recipients.into_iter().filter(|user| *user != who.id) {
            let event = DocumentSharedEvent {
                document_id: document.id.clone(),
                document_name: document.name.clone(),
                recipient_user_id: recipient.clone(),
                subject_kind: grant.subject_kind.clone(),
                subject_value: grant.subject_value.clone(),
                level: grant.access_level,
                by: grant.granted_by.clone(),
                shared_by: who.username.clone(),
                message: message.clone(),
                v: SHARED_EVENT_VERSION,
                via_agent: cause.via_agent.clone(),
                comment_id: cause.comment_id.clone(),
                created_at: Some(grant.updated_at),
            };
            if let Err(error) = self
                .events
                .shared(std::slice::from_ref(&recipient), &event)
                .await
            {
                tracing::warn!(document_id = %document.id, error = %error, "Share event not relayed");
            }
        }
    }
}

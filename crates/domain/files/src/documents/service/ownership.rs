//! Reading a document's access back, ownership transfer, revoking every
//! grant at once, and the `document.shared` event.
//!
//! Transferring ownership and revoking every grant are the owner's alone;
//! the previous owner keeps an `edit` grant. The recipient of a transfer is
//! validated by the caller layer, which resolves it to an active user and
//! hands its display name in [`TransferRequest`]. A caller below `share`
//! reads only the owner and their own level, never the other subjects.
//!
//! `document.shared` is addressed per recipient: a `user` subject is the
//! user, a role or dimension subject is resolved to its members at emit time
//! (capped, with the truncation logged). The sharer is never a recipient.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, DocumentAction, DocumentOwnershipTransferredEvent, DocumentSharedEvent,
};

use super::DocumentService;
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentGrant, GrantTarget, NewGrant};

pub(super) const USER_SUBJECT: &str = "user";

/// Who holds what on one document: its owner, the grants on the document,
/// the grants it inherits from its folder chain, and the caller's level.
/// Both grant lists are empty for a caller below `share`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentAccess {
    pub document: Document,
    pub owner: UserId,
    pub grants: Vec<DocumentGrant>,
    pub inherited: Vec<DocumentGrant>,
    pub my_access: AccessLevel,
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
    pub async fn access_of(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<DocumentAccess, DocumentsError> {
        let granted = self
            .access
            .authorize(
                who,
                AccessTarget::Document(document.clone()),
                DocumentAction::View,
                trace,
            )
            .await?;
        let current = granted
            .document
            .ok_or_else(|| DocumentsError::not_found("document", document))?;
        let (grants, inherited) = if granted.level >= AccessLevel::Share {
            self.grants_with_inherited(&current).await?
        } else {
            (Vec::new(), Vec::new())
        };
        Ok(DocumentAccess {
            owner: current.owner_user_id.clone(),
            document: current,
            grants,
            inherited,
            my_access: granted.level,
        })
    }

    async fn grants_with_inherited(
        &self,
        document: &Document,
    ) -> Result<(Vec<DocumentGrant>, Vec<DocumentGrant>), DocumentsError> {
        let grants = self.repos.grants.list_for_document(&document.id).await?;
        let inherited = match &document.folder_id {
            Some(folder) => {
                let mut chain = vec![folder.clone()];
                chain.extend(self.repos.folders.ancestors(folder).await?);
                self.repos.grants.list_for_folders(&chain).await?
            },
            None => Vec::new(),
        };
        Ok((grants, inherited))
    }

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
        if !self.repos.documents.set_owner(document, &to).await? {
            return Err(DocumentsError::not_found("document", document));
        }
        if current.owner_user_id != to {
            self.repos
                .grants
                .insert(NewGrant {
                    target: GrantTarget::Document(document.clone()),
                    subject_kind: USER_SUBJECT.to_owned(),
                    subject_value: current.owner_user_id.as_str().to_owned(),
                    subject_display: Some(who.username.clone()),
                    access_level: AccessLevel::Edit,
                    granted_by: who.id.clone(),
                    valid_until: None,
                })
                .await?;
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
        let revoked = self.repos.grants.delete_for_document(document).await?;
        for grant in &grants {
            self.clear_presence(grant, Some(&current.owner_user_id))
                .await;
        }
        Ok(revoked)
    }

    pub(super) async fn announce_share(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        grant: &DocumentGrant,
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

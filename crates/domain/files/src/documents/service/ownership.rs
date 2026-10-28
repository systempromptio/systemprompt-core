//! Reading a document's access back, ownership transfer, revoking every
//! grant at once, and the `document.shared` event.
//!
//! Transferring ownership needs `own`; the previous owner keeps an `edit`
//! grant. Revoking every grant needs `own` too, since it can remove grants
//! at any level.
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

/// Who holds what on one document: its owner, the grants on the document,
/// the grants it inherits from its folder chain, and the caller's level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentAccess {
    pub document: Document,
    pub owner: UserId,
    pub grants: Vec<DocumentGrant>,
    pub inherited: Vec<DocumentGrant>,
    pub my_access: AccessLevel,
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
        let grants = self.repos.grants.list_for_document(document).await?;
        let inherited = match &current.folder_id {
            Some(folder) => {
                let mut chain = vec![folder.clone()];
                chain.extend(self.repos.folders.ancestors(folder).await?);
                self.repos.grants.list_for_folders(&chain).await?
            },
            None => Vec::new(),
        };
        Ok(DocumentAccess {
            owner: current.owner_user_id.clone(),
            document: current,
            grants,
            inherited,
            my_access: granted.level,
        })
    }

    pub async fn transfer_ownership(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        to: &UserId,
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        let current = self
            .authorized_document(who, document, DocumentAction::Own, trace)
            .await?;
        if !self.repos.documents.set_owner(document, to).await? {
            return Err(DocumentsError::not_found("document", document));
        }
        if current.owner_user_id != *to {
            self.repos
                .grants
                .insert(NewGrant {
                    target: GrantTarget::Document(document.clone()),
                    subject_kind: "user".to_owned(),
                    subject_value: current.owner_user_id.as_str().to_owned(),
                    subject_display: (who.id == current.owner_user_id)
                        .then(|| who.username.clone()),
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
        if let Err(error) = self
            .events
            .ownership_transferred(std::slice::from_ref(to), &event)
            .await
        {
            tracing::warn!(document_id = %document, error = %error, "Ownership transfer event not relayed");
        }
        self.load_document(document).await
    }

    pub async fn revoke_all(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<u64, DocumentsError> {
        self.authorized_document(who, document, DocumentAction::Own, trace)
            .await?;
        Ok(self.repos.grants.delete_for_document(document).await?)
    }

    pub(super) async fn announce_share(&self, document: &Document, grant: &DocumentGrant) {
        let event = DocumentSharedEvent {
            document_id: document.id.clone(),
            subject_kind: grant.subject_kind.clone(),
            subject_value: grant.subject_value.clone(),
            level: grant.access_level,
            by: grant.granted_by.clone(),
        };
        let mut to = self.recipients(document).await;
        if grant.subject_kind == "user" {
            let subject = UserId::new(grant.subject_value.clone());
            if !to.contains(&subject) {
                to.push(subject);
            }
        }
        if let Err(error) = self.events.shared(&to, &event).await {
            tracing::warn!(document_id = %document.id, error = %error, "Share event not relayed");
        }
    }
}

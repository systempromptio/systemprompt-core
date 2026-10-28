//! Sharing: grants on documents and folders.
//!
//! Sharing needs `share` on the target, and nobody can grant a level above
//! the one they hold. A grant names one subject pair: `user`, `role` or a
//! registered subject dimension; any other kind is refused rather than
//! stored as a grant nobody can match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction};
use uuid::Uuid;

use super::DocumentService;
use crate::documents::access::{AccessTarget, Granted};
use crate::documents::error::DocumentsError;
use crate::documents::models::{DocumentGrant, GrantTarget, NewGrant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRequest {
    pub target: GrantTarget,
    pub subject_kind: String,
    pub subject_value: String,
    pub subject_display: Option<String>,
    pub level: AccessLevel,
    pub valid_until: Option<DateTime<Utc>>,
}

pub(super) fn owner_of(granted: &Granted) -> Option<UserId> {
    granted
        .document
        .as_ref()
        .map(|d| d.owner_user_id.clone())
        .or_else(|| granted.folder.as_ref().map(|f| f.owner_user_id.clone()))
}

fn access_target(target: &GrantTarget) -> AccessTarget {
    match target {
        GrantTarget::Document(id) => AccessTarget::Document(id.clone()),
        GrantTarget::Folder(id) => AccessTarget::Folder(Some(id.clone())),
    }
}

impl DocumentService {
    pub(super) async fn authorize_share(
        &self,
        who: &AuthenticatedUser,
        target: &GrantTarget,
        trace: &TraceId,
    ) -> Result<Granted, DocumentsError> {
        self.access
            .authorize(who, access_target(target), DocumentAction::Share, trace)
            .await
    }

    pub async fn share(
        &self,
        who: &AuthenticatedUser,
        request: ShareRequest,
        trace: &TraceId,
    ) -> Result<DocumentGrant, DocumentsError> {
        if !self.access.is_known_subject_kind(&request.subject_kind) {
            return Err(DocumentsError::UnknownSubjectKind {
                kind: request.subject_kind,
            });
        }
        let granted = self.authorize_share(who, &request.target, trace).await?;
        if request.level > granted.level {
            return Err(DocumentsError::Forbidden {
                action: DocumentAction::Share,
                needed: request.level,
                held: Some(granted.level),
                policy: None,
                owner: owner_of(&granted),
            });
        }
        let grant = self
            .repos
            .grants
            .insert(NewGrant {
                target: request.target,
                subject_kind: request.subject_kind,
                subject_value: request.subject_value,
                subject_display: request.subject_display,
                access_level: request.level,
                granted_by: who.id.clone(),
                valid_until: request.valid_until,
            })
            .await?;
        if let Some(document) = &granted.document {
            self.announce_share(document, &grant).await;
        }
        Ok(grant)
    }

    pub async fn revoke(
        &self,
        who: &AuthenticatedUser,
        target: &GrantTarget,
        grant_id: Uuid,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        let granted = self.authorize_share(who, target, trace).await?;
        let grant = self
            .list_grants(target)
            .await?
            .into_iter()
            .find(|grant| grant.id == grant_id)
            .ok_or_else(|| DocumentsError::not_found("grant", grant_id))?;
        if grant.access_level > granted.level {
            return Err(DocumentsError::Forbidden {
                action: DocumentAction::Share,
                needed: grant.access_level,
                held: Some(granted.level),
                policy: None,
                owner: owner_of(&granted),
            });
        }
        if !self.repos.grants.delete(grant_id).await? {
            return Err(DocumentsError::not_found("grant", grant_id));
        }
        Ok(())
    }

    pub async fn grants(
        &self,
        who: &AuthenticatedUser,
        target: &GrantTarget,
        trace: &TraceId,
    ) -> Result<Vec<DocumentGrant>, DocumentsError> {
        self.authorize_share(who, target, trace).await?;
        self.list_grants(target).await
    }

    pub(super) async fn list_grants(
        &self,
        target: &GrantTarget,
    ) -> Result<Vec<DocumentGrant>, DocumentsError> {
        Ok(match target {
            GrantTarget::Document(id) => self.repos.grants.list_for_document(id).await?,
            GrantTarget::Folder(id) => self.repos.grants.list_for_folder(id).await?,
        })
    }
}

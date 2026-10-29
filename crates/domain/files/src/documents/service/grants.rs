//! Sharing: grants on documents and folders.
//!
//! Sharing needs `share` on the target, and nobody can grant a level above
//! the one they hold, nor change a grant whose subject already holds more
//! than they do. `own` is never granted: it stays with the owner. A grant
//! names one subject pair: `user`, `role` or a registered subject dimension;
//! any other kind is refused rather than stored as a grant nobody can match.
//!
//! The subject must exist: an active user, a role in the role catalogue, or
//! a value the dimension's provider lists (any value when it cannot list
//! them). A share may carry a message of at most [`MAX_SHARE_MESSAGE_BYTES`]
//! for the `document.shared` event. Every grant change writes a row to the
//! grant audit log in the same statement.
//!
//! Removing a grant also removes the presence rows (and so the live editing
//! state) of the users it named, so a revoked user stops receiving the
//! document's events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction};
use uuid::Uuid;

use systemprompt_traits::RepositoryError;

use super::DocumentService;
use super::ownership::USER_SUBJECT;
use crate::documents::access::{AccessTarget, Granted};
use crate::documents::error::DocumentsError;
use crate::documents::models::{DocumentGrant, GrantActor, GrantTarget, NewGrant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRequest {
    pub target: GrantTarget,
    pub subject_kind: String,
    pub subject_value: String,
    pub subject_display: Option<String>,
    pub level: AccessLevel,
    pub valid_until: Option<DateTime<Utc>>,
    pub message: Option<String>,
    pub via_agent: Option<String>,
}

pub const MAX_SHARE_MESSAGE_BYTES: usize = 1024;

pub(super) struct CheckedShare {
    pub(super) grant: NewGrant,
    pub(super) granted: Granted,
    pub(super) recipients: Vec<UserId>,
    pub(super) message: Option<String>,
    pub(super) via_agent: Option<String>,
}

const AGENT_SHARE: &str = "shared by agent";

pub(super) fn owner_of(granted: &Granted) -> Option<UserId> {
    granted
        .document
        .as_ref()
        .map(|d| d.owner_user_id.clone())
        .or_else(|| granted.folder.as_ref().map(|f| f.owner_user_id.clone()))
}

pub(super) fn check_message(message: Option<&str>) -> Result<(), DocumentsError> {
    match message {
        Some(message) if message.len() > MAX_SHARE_MESSAGE_BYTES => Err(DocumentsError::TooLarge {
            what: "share message",
            size: u64::try_from(message.len()).unwrap_or(u64::MAX),
            limit: MAX_SHARE_MESSAGE_BYTES as u64,
        }),
        _ => Ok(()),
    }
}

pub(super) fn access_target(target: &GrantTarget) -> AccessTarget {
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
        let checked = self.check_share(who, request, trace).await?;
        let mut tx = self.repos.documents.begin().await?;
        let grant = self
            .repos
            .grants
            .insert(&mut tx, checked.grant.clone())
            .await?;
        self.record_checked(&mut tx, who, &checked, &grant).await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        self.announce_checked(who, checked, &grant).await;
        Ok(grant)
    }

    pub(super) async fn check_share(
        &self,
        who: &AuthenticatedUser,
        request: ShareRequest,
        trace: &TraceId,
    ) -> Result<CheckedShare, DocumentsError> {
        if !self.access.is_known_subject_kind(&request.subject_kind) {
            return Err(DocumentsError::UnknownSubjectKind {
                kind: request.subject_kind,
            });
        }
        if request.level == AccessLevel::Own {
            return Err(DocumentsError::OwnNotGrantable);
        }
        check_message(request.message.as_deref())?;
        let granted = self.authorize_share(who, &request.target, trace).await?;
        self.access
            .validate_subject(&request.subject_kind, &request.subject_value)
            .await?;
        let existing = self
            .list_grants(&request.target)
            .await?
            .into_iter()
            .find(|grant| {
                grant.subject_kind == request.subject_kind
                    && grant.subject_value == request.subject_value
            });
        if let Some(existing) = existing.filter(|grant| grant.access_level > granted.level) {
            return Err(DocumentsError::ShareBelowExisting {
                existing: existing.access_level,
                held: granted.level,
                owner: owner_of(&granted),
            });
        }
        if request.level > granted.level {
            return Err(DocumentsError::Forbidden {
                action: DocumentAction::Share,
                needed: request.level,
                held: Some(granted.level),
                policy: None,
                owner: owner_of(&granted),
            });
        }
        let recipients = match &granted.document {
            Some(_) => {
                self.share_recipients(who, &request.subject_kind, &request.subject_value)
                    .await?
            },
            None => Vec::new(),
        };
        Ok(CheckedShare {
            grant: NewGrant {
                target: request.target,
                subject_kind: request.subject_kind,
                subject_value: request.subject_value,
                subject_display: request.subject_display,
                access_level: request.level,
                granted_by: who.id.clone(),
                valid_until: request.valid_until,
                trace_id: trace.clone(),
                reason: request.via_agent.as_ref().map(|_| AGENT_SHARE.to_owned()),
                via_agent: request.via_agent.clone(),
            },
            granted,
            recipients,
            message: request.message,
            via_agent: request.via_agent,
        })
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
        let actor = GrantActor {
            user: who.id.clone(),
            trace: trace.clone(),
        };
        if !self.repos.grants.delete(grant_id, &actor).await? {
            return Err(DocumentsError::not_found("grant", grant_id));
        }
        self.clear_presence(&grant, owner_of(&granted).as_ref())
            .await;
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

    pub(super) async fn clear_presence(&self, grant: &DocumentGrant, owner: Option<&UserId>) {
        let users = if grant.subject_kind == USER_SUBJECT {
            vec![UserId::new(grant.subject_value.clone())]
        } else {
            match self
                .access
                .subject_members(&grant.subject_kind, &grant.subject_value)
                .await
            {
                Ok(users) => users,
                Err(error) => {
                    tracing::warn!(grant_id = %grant.id, error = %error, "Revoked subject's users were not resolved; their presence stays until it expires");
                    return;
                },
            }
        };
        let users: Vec<UserId> = users
            .into_iter()
            .filter(|user| Some(user) != owner)
            .collect();
        if users.is_empty() {
            return;
        }
        let cleared = match &grant.target {
            GrantTarget::Document(id) => self.repos.presence.delete_for_users(id, &users).await,
            GrantTarget::Folder(id) => self.repos.presence.delete_for_users_under(id, &users).await,
        };
        if let Err(error) = cleared {
            tracing::warn!(grant_id = %grant.id, error = %error, "Revoked users' presence was not cleared; it stays until it expires");
        }
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

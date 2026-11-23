//! Inviting an email outside the instance to a document, and claiming the
//! invite.
//!
//! An invite is created, listed and revoked under the same rules as a public
//! link (`share` on the document, never an agent, the external-sharing
//! policy for invites), and is addressed to one email at `view` or
//! `comment`. Core sends nothing: the invite's token is returned once to the
//! sharer, who delivers the link. The link reads the document anonymously.
//!
//! A claim turns the invite into an ordinary `user` grant at its level. Only
//! a signed-in person whose verified email is the invite's may claim it, only
//! while it is open and the policy still allows invites; a mismatch is the
//! same refusal as an unknown invite. The grant never lowers a level the
//! person already holds, and the Drive hint follows it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use systemprompt_identifiers::{DocumentId, DocumentInviteId, Email, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::AccessLevel;
use systemprompt_traits::RepositoryError;

use super::DocumentService;
use super::drive_hint::DriveHint;
use super::external::{new_token, token_hash, well_formed_token};
use super::ownership::USER_SUBJECT;
use crate::config::ExternalChannel;
use crate::documents::error::DocumentsError;
use crate::documents::models::{DocumentInvite, GrantActor, GrantTarget, NewGrant, NewInvite};

const INVITE_REASON: &str = "external invite";

#[derive(Debug, Clone)]
pub struct InviteRequest {
    pub email: String,
    pub level: AccessLevel,
    pub days: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct CreatedInvite {
    pub invite: DocumentInvite,
    pub token: String,
}

fn invite_email(raw: &str) -> Result<Email, DocumentsError> {
    Email::try_new(raw.trim().to_lowercase()).map_err(|_invalid| DocumentsError::InvalidParams {
        detail: "an invite needs one valid email address",
    })
}

fn unknown_invite() -> DocumentsError {
    DocumentsError::not_found("invite", "token")
}

impl DocumentService {
    pub async fn create_invite(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        request: InviteRequest,
        trace: &TraceId,
    ) -> Result<CreatedInvite, DocumentsError> {
        if !matches!(request.level, AccessLevel::View | AccessLevel::Comment) {
            return Err(DocumentsError::InvalidParams {
                detail: "an invite grants view or comment",
            });
        }
        let email = invite_email(&request.email)?;
        let document = self
            .shareable_externally(who, document, ExternalChannel::Invite, trace)
            .await?;
        let days = self.config().external_sharing.link_days(request.days);
        let token = new_token();
        let invite = self
            .repos
            .invites
            .create(&NewInvite {
                document_id: document.id,
                email,
                level: request.level,
                token_hash: token_hash(&token),
                expires_at: Utc::now() + Duration::days(i64::from(days)),
                actor: GrantActor {
                    user: who.id.clone(),
                    trace: trace.clone(),
                },
            })
            .await?;
        Ok(CreatedInvite { invite, token })
    }

    pub async fn list_invites(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<Vec<DocumentInvite>, DocumentsError> {
        self.refuse_agent_external()?;
        self.authorize_share(who, &GrantTarget::Document(document.clone()), trace)
            .await?;
        Ok(self.repos.invites.list(document).await?)
    }

    pub async fn revoke_invite(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        invite: &DocumentInviteId,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.refuse_agent_external()?;
        self.authorize_share(who, &GrantTarget::Document(document.clone()), trace)
            .await?;
        let actor = GrantActor {
            user: who.id.clone(),
            trace: trace.clone(),
        };
        if self.repos.invites.revoke(document, invite, &actor).await? {
            Ok(())
        } else {
            Err(DocumentsError::not_found("invite", invite))
        }
    }

    pub async fn claim_invite(
        &self,
        who: &AuthenticatedUser,
        token: &str,
        verified_email: Option<&Email>,
        trace: &TraceId,
    ) -> Result<DocumentId, DocumentsError> {
        self.refuse_agent_external()?;
        if !well_formed_token(token) {
            return Err(unknown_invite());
        }
        let invite = self
            .repos
            .invites
            .by_token(&token_hash(token))
            .await?
            .filter(|invite| invite.is_open(Utc::now()))
            .filter(|invite| verified_email.is_some_and(|email| *email == invite.email))
            .ok_or_else(unknown_invite)?;
        let document = self
            .repos
            .documents
            .find(&invite.document_id)
            .await?
            .ok_or_else(unknown_invite)?;
        if !self
            .external_allowed(ExternalChannel::Invite, &document)
            .await?
        {
            return Err(DocumentsError::ExternalSharingRefused { channel: "invite" });
        }
        let actor = GrantActor {
            user: who.id.clone(),
            trace: trace.clone(),
        };
        let grant = NewGrant {
            target: GrantTarget::Document(document.id.clone()),
            subject_kind: USER_SUBJECT.to_owned(),
            subject_value: who.id.as_str().to_owned(),
            subject_display: None,
            access_level: invite.level,
            granted_by: invite.invited_by.clone(),
            valid_until: None,
            trace_id: trace.clone(),
            reason: Some(INVITE_REASON.to_owned()),
            via_agent: None,
        };
        let mut tx = self.repos.documents.begin().await?;
        self.repos
            .grants
            .insert_bounded(&mut tx, grant, invite.level)
            .await?;
        if !self
            .repos
            .invites
            .mark_claimed(&mut tx, &invite.id, &actor)
            .await?
        {
            return Err(unknown_invite());
        }
        tx.commit().await.map_err(RepositoryError::from)?;
        self.hint_drive_changed(DriveHint::document(&document, &who.id));
        Ok(document.id)
    }
}

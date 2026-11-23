//! Sharing a document outside the instance: a read-only public link.
//!
//! Creating, listing and revoking a link needs `share` on the document,
//! exactly as a grant does, and is refused to every agent whatever its cap:
//! only a person hands a document to someone the instance does not know.
//! The `files.documents.externalSharing` policy must allow the channel, the
//! document's format and every folder above it, both when the link is made
//! and on every open.
//!
//! The token is 256 random bits, returned once and stored only as its
//! SHA-256. An optional password is stored only as its bcrypt hash. An open
//! that does not resolve to a live link of an allowed, untrashed document
//! with the right password is refused alike (see `external_open`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, PublicLinkId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;

use super::DocumentService;
use crate::config::ExternalChannel;
use crate::documents::agents::{AgentCapability, AgentRefusal};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, GrantActor, GrantTarget, NewPublicLink, PublicLink};

const MIN_LINK_PASSWORD_CHARS: usize = 8;
pub(super) const MAX_LINK_PASSWORD_BYTES: usize = 72;

const TOKEN_BYTES: usize = 32;
const TOKEN_CHARS: usize = 43;

#[derive(Debug, Clone, Default)]
pub struct PublicLinkRequest {
    pub password: Option<String>,
    pub days: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct CreatedPublicLink {
    pub link: PublicLink,
    pub token: String,
}

pub(super) fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub(super) fn new_token() -> String {
    let mut raw = [0u8; TOKEN_BYTES];
    rand::rng().fill_bytes(&mut raw);
    URL_SAFE_NO_PAD.encode(raw)
}

pub(super) fn well_formed_token(token: &str) -> bool {
    token.len() == TOKEN_CHARS
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl DocumentService {
    pub(super) fn refuse_agent_external(&self) -> Result<(), DocumentsError> {
        self.access.actor().as_agent().map_or(Ok(()), |agent| {
            Err(AgentRefusal::HardRule {
                agent: agent.name.clone(),
                capability: AgentCapability::Share,
            }
            .into())
        })
    }

    pub(super) async fn external_allowed(
        &self,
        channel: ExternalChannel,
        document: &Document,
    ) -> Result<bool, DocumentsError> {
        let policy = &self.config().external_sharing;
        if !policy.channel_enabled(channel) {
            return Ok(false);
        }
        let path: Vec<DocumentFolderId> = match &document.folder_id {
            None => Vec::new(),
            Some(id) => match self.repos.folders.find(id).await? {
                Some(folder) => folder
                    .ancestor_ids
                    .into_iter()
                    .chain(std::iter::once(folder.id))
                    .collect(),
                None => return Ok(false),
            },
        };
        Ok(policy.allows(channel, document.format, &path))
    }

    pub(super) async fn shareable_externally(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        channel: ExternalChannel,
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        self.refuse_agent_external()?;
        let granted = self
            .authorize_share(who, &GrantTarget::Document(document.clone()), trace)
            .await?;
        let document = granted
            .document
            .ok_or_else(|| DocumentsError::not_found("document", document))?;
        if !self.external_allowed(channel, &document).await? {
            return Err(DocumentsError::ExternalSharingRefused {
                channel: channel_name(channel),
            });
        }
        Ok(document)
    }

    pub async fn create_public_link(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        request: PublicLinkRequest,
        trace: &TraceId,
    ) -> Result<CreatedPublicLink, DocumentsError> {
        let document = self
            .shareable_externally(who, document, ExternalChannel::PublicLink, trace)
            .await?;
        let password_hash = match request.password {
            None => None,
            Some(password) => Some(hash_password(password).await?),
        };
        let days = self.config().external_sharing.link_days(request.days);
        let token = new_token();
        let link = self
            .repos
            .public_links
            .create(&NewPublicLink {
                document_id: document.id,
                token_hash: token_hash(&token),
                password_hash,
                expires_at: Utc::now() + Duration::days(i64::from(days)),
                actor: GrantActor {
                    user: who.id.clone(),
                    trace: trace.clone(),
                },
            })
            .await?;
        Ok(CreatedPublicLink { link, token })
    }

    pub async fn list_public_links(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<Vec<PublicLink>, DocumentsError> {
        self.refuse_agent_external()?;
        self.authorize_share(who, &GrantTarget::Document(document.clone()), trace)
            .await?;
        Ok(self.repos.public_links.list(document).await?)
    }

    pub async fn revoke_public_link(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        link: &PublicLinkId,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.refuse_agent_external()?;
        self.authorize_share(who, &GrantTarget::Document(document.clone()), trace)
            .await?;
        let actor = GrantActor {
            user: who.id.clone(),
            trace: trace.clone(),
        };
        if self
            .repos
            .public_links
            .revoke(document, link, &actor)
            .await?
        {
            Ok(())
        } else {
            Err(DocumentsError::not_found("public link", link))
        }
    }
}

pub(super) const fn channel_name(channel: ExternalChannel) -> &'static str {
    match channel {
        ExternalChannel::PublicLink => "public link",
        ExternalChannel::Invite => "invite",
    }
}

async fn hash_password(password: String) -> Result<String, DocumentsError> {
    if password.chars().count() < MIN_LINK_PASSWORD_CHARS {
        return Err(DocumentsError::InvalidParams {
            detail: "a link password needs at least 8 characters",
        });
    }
    if password.len() > MAX_LINK_PASSWORD_BYTES {
        return Err(DocumentsError::InvalidParams {
            detail: "a link password may be at most 72 bytes",
        });
    }
    tokio::task::spawn_blocking(move || bcrypt::hash(password, bcrypt::DEFAULT_COST))
        .await?
        .map_err(|source| DocumentsError::LinkPassword { source })
}

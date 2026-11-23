//! A document shared outside the instance: a read-only public link, as its
//! sharer sees it, and the stored gate an anonymous open is checked against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use systemprompt_identifiers::{DocumentId, PublicLinkId, UserId};

use super::GrantActor;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicLink {
    pub id: PublicLinkId,
    pub document_id: DocumentId,
    pub has_password: bool,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub open_count: i64,
    pub last_opened_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewPublicLink {
    pub document_id: DocumentId,
    pub token_hash: String,
    pub password_hash: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub actor: GrantActor,
}

#[derive(Debug, Clone)]
pub struct PublicLinkGate {
    pub id: PublicLinkId,
    pub document_id: DocumentId,
    pub password_hash: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl PublicLinkGate {
    #[must_use]
    pub fn is_live(&self, now: DateTime<Utc>) -> bool {
        self.revoked_at.is_none() && self.expires_at > now
    }
}

//! The Drive sync token view.
//!
//! The Drive sync routes act on a short token whose only audience is the
//! Drive sync audience. [`DriveTokens`] is the narrow view those routes hold
//! in place of the token authority: it mints a Drive sync token for one
//! subject and verifies a presented Drive sync token back to its subject, and
//! signs or verifies no other token kind. The member that owns the documents
//! capability builds it from the sealed token authority in its provide and
//! confers it on `gateway-http`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{JwtToken, SessionId, UserId};

use crate::BoxedSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveTokenSubject {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub not_after: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct IssuedDriveToken {
    pub token: JwtToken,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum DriveTokenError {
    #[error("the subject's lifetime has ended")]
    Expired,
    #[error("the presented token is not a Drive sync token")]
    Rejected(#[source] BoxedSource),
    #[error("the Drive sync token was not minted")]
    Mint(#[source] BoxedSource),
}

/// Mints and verifies Drive sync tokens, and nothing else.
///
/// `mint` signs a token whose only audience is the Drive sync audience for
/// `subject`, expiring no later than `subject.not_after`; `verify` accepts
/// only such a token and returns the subject it was minted for.
pub trait DriveTokens: Send + Sync + std::fmt::Debug {
    fn mint(&self, subject: &DriveTokenSubject) -> Result<IssuedDriveToken, DriveTokenError>;

    fn verify(&self, token: &JwtToken) -> Result<DriveTokenSubject, DriveTokenError>;
}

pub type DynDriveTokens = Arc<dyn DriveTokens>;

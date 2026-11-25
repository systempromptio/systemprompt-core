//! [`ScopedTokenMinter`]: the token mint core confers on a member, beside the
//! [`TokenAuthority`](super::TokenAuthority) it signs with.
//!
//! A member mints only for an audience it declared at contribution, with
//! scopes it declared for that audience, for at most
//! [`SCOPED_TOKEN_MAX_TTL_SECS`]. Admission refuses a declared audience that
//! is a core audience ([`is_core_audience`]) or is not in the member's own
//! namespace (`<member-id>:<name>`), and a declared scope that is a core
//! permission ([`is_core_scope`]), so no minted token carries the admin
//! scope or names an audience a core verifier accepts. The system admin is
//! never a minted token's subject, and the audit row's actor is the member,
//! not the subject. The mint re-checks
//! the request against the grant and records every mint in the governance
//! decision audit before the token is returned; a failed audit write fails
//! the mint. Core verifiers are unchanged and accept only core audiences;
//! the member's own minter is the one verifier of its tokens
//! ([`ScopedTokenMinter::verify`]): it accepts a token only for an audience
//! that member declared, signed under the deployment's keys and issuer, whose
//! `aud` is exactly that audience, whose `azp` is that member, whose scopes
//! were declared for it, and which has not expired (no leeway). A core token,
//! another member's token and an expired token are refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::{AccessTokenId, UserId};
use systemprompt_traits::BoxedSource;

use super::{JwtAudience, Permission};

pub const SCOPED_TOKEN_MAX_TTL_SECS: i64 = 3600;

#[derive(Debug, Clone)]
pub struct ScopedTokenRequest {
    pub subject: UserId,
    pub audience: String,
    pub scopes: Vec<String>,
    pub ttl_secs: i64,
}

#[derive(Debug, Clone)]
pub struct ScopedToken {
    pub token: String,
    pub jti: AccessTokenId,
    pub expires_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ScopedMintError {
    #[error("audience {0} is not one this member declared")]
    UndeclaredAudience(String),
    #[error("scope {scope} is not declared for audience {audience}")]
    UndeclaredScope { audience: String, scope: String },
    #[error("the system admin is never the subject of a conferred token")]
    ReservedSubject,
    #[error("a scoped token lives between 1 and {SCOPED_TOKEN_MAX_TTL_SECS} seconds (got {0})")]
    Lifetime(i64),
    #[error("the token could not be signed")]
    Signing(#[source] BoxedSource),
    #[error("the mint could not be audited, so no token was issued")]
    Audit(#[source] BoxedSource),
}

/// What a verified scoped token carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedClaims {
    pub subject: UserId,
    pub audience: String,
    pub scopes: Vec<String>,
    pub jti: AccessTokenId,
    pub issued_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ScopedVerifyError {
    #[error("audience {0} is not one this member declared")]
    UndeclaredAudience(String),
    #[error("the token has expired")]
    Expired,
    #[error("the token was not minted by this member")]
    OtherMember,
    #[error("the token carries a scope not declared for audience {0}")]
    UndeclaredScope(String),
    #[error("the token did not verify for this audience")]
    NotAuthentic(#[source] BoxedSource),
}

/// Mints and verifies one member's tokens for its declared audiences.
/// `#[async_trait]` keeps the trait `dyn`-compatible for its issuer.
#[async_trait]
pub trait ScopedTokenMinter: Send + Sync + std::fmt::Debug {
    async fn mint(&self, request: &ScopedTokenRequest) -> Result<ScopedToken, ScopedMintError>;

    fn verify(&self, audience: &str, token: &str) -> Result<ScopedClaims, ScopedVerifyError>;
}

const CORE_AUDIENCES: &[JwtAudience] = &[
    JwtAudience::Web,
    JwtAudience::Api,
    JwtAudience::A2a,
    JwtAudience::Mcp,
    JwtAudience::Internal,
    JwtAudience::Bridge,
    JwtAudience::Hook,
    JwtAudience::Ui,
];

#[must_use]
pub fn is_core_audience(audience: &str) -> bool {
    CORE_AUDIENCES
        .iter()
        .any(|core| core.as_str().eq_ignore_ascii_case(audience))
}

#[must_use]
pub fn is_core_scope(scope: &str) -> bool {
    Permission::ALL_VARIANTS
        .iter()
        .any(|core| core.eq_ignore_ascii_case(scope))
}

//! The deployment's token authority: verification and signing of its own
//! RS256 JWTs behind one object-safe trait.
//!
//! Extension code holds only `&dyn ExtensionContext`; it reaches the
//! deployment's keys and issuer through [`TokenAuthority`] and never through
//! the key material itself. The trait is `dyn`-compatible on purpose: the
//! implementor lives in the infra layer (`systemprompt_security`), the
//! consumers live in domain, app and extension crates that must not depend on
//! it, and the object is handed across the `ExtensionContext` seam.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Duration;
use systemprompt_identifiers::{AccessTokenId, ClientId, PluginId, SessionId};
use systemprompt_traits::BoxedSource;
use thiserror::Error;

use super::{AuthenticatedUser, JwtAudience, JwtClaims, Permission};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TokenError {
    #[error("token `kid` `{0}` does not match any known signing key")]
    UnknownKid(String),

    #[error("token carries no `kid` naming the active signing key")]
    KidMismatch,

    #[error("token has expired")]
    Expired,

    #[error("token signature is invalid")]
    InvalidSignature,

    #[error("token audience is not accepted")]
    InvalidAudience,

    #[error("token issuer is not this deployment")]
    InvalidIssuer,

    #[error("token is malformed: {0}")]
    Malformed(#[source] BoxedSource),

    #[error("token rejected by policy: {0}")]
    Rejected(#[source] BoxedSource),

    #[error("token lifetime of {seconds}s is outside the permitted range")]
    InvalidLifetime { seconds: i64 },

    #[error("token signing failed: {0}")]
    Signing(#[source] BoxedSource),
}

pub type TokenResult<T> = Result<T, TokenError>;

#[derive(Debug, Clone)]
pub struct UserJwtRequest {
    pub user: AuthenticatedUser,
    pub session_id: SessionId,
    pub jti: AccessTokenId,
    pub permissions: Vec<Permission>,
    pub audience: Vec<JwtAudience>,
    pub ttl: Duration,
    pub client_id: Option<ClientId>,
    pub purpose: Option<String>,
    pub plugin_id: Option<PluginId>,
}

pub trait TokenAuthority: Send + Sync + std::fmt::Debug {
    fn verify_session_cookie(&self, token: &str) -> TokenResult<JwtClaims>;

    fn verify_bearer(&self, token: &str, audiences: &[JwtAudience]) -> TokenResult<JwtClaims>;

    fn verify_hook_token(&self, token: &str) -> TokenResult<JwtClaims>;

    fn sign_claims(&self, claims: &serde_json::Value) -> TokenResult<String>;

    fn mint_user_jwt(&self, req: &UserJwtRequest) -> TokenResult<String>;
}

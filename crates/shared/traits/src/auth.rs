//! Authentication and role-management provider traits.
//!
//! `UserProvider` is dispatched as a trait object (`dyn UserProvider`), so it
//! uses `#[async_trait]`; native `async fn` in traits is not yet
//! `dyn`-compatible. `RoleProvider` is only used through concrete types and
//! declares native `async` methods.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::future::Future;
use systemprompt_identifiers::UserId;

use crate::BoxedSource;

pub type AuthResult<T> = Result<T, AuthProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AuthProviderError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("User not found")]
    UserNotFound,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Insufficient permissions")]
    InsufficientPermissions,

    #[error("Internal error: {0}")]
    Internal(#[source] BoxedSource),
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
    pub is_active: bool,
}

/// Federated-identity claim payload passed to
/// [`UserProvider::find_or_create_federated`].
///
/// Carries only the OIDC fields needed to seed a freshly federated user — the
/// trait stays free of any concrete JWT type so it can live in
/// `systemprompt-traits` without taking a dependency on `systemprompt-models`.
#[derive(Debug, Clone, Default)]
pub struct FederatedIdentityClaims {
    pub email: Option<String>,
    pub email_verified: bool,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    pub roles: Vec<String>,
}

/// Whether an inbound chat-platform sender resolves to a linkable identity.
///
/// This is the identity-linking rule, stated once: a sender is `Linked` only
/// when the platform verified the claims (for Slack, a workspace profile with
/// a confirmed email — an unconfirmed address would let anyone who can set it
/// claim the account that owns it). Anything less is `Unlinked`, whose empty
/// claims land the sender on a fresh, role-less first-touch user that no rule
/// grants anything to — never on an existing account.
#[derive(Debug, Clone, Default)]
pub enum SenderIdentity {
    Linked(FederatedIdentityClaims),
    #[default]
    Unlinked,
}

impl SenderIdentity {
    #[must_use]
    pub fn claims(&self) -> FederatedIdentityClaims {
        match self {
            Self::Linked(claims) => claims.clone(),
            Self::Unlinked => FederatedIdentityClaims::default(),
        }
    }
}

#[async_trait]
pub trait UserProvider: Send + Sync {
    async fn find_by_id(&self, id: &UserId) -> AuthResult<Option<AuthUser>>;
    async fn find_by_email(&self, email: &str) -> AuthResult<Option<AuthUser>>;
    async fn find_by_name(&self, name: &str) -> AuthResult<Option<AuthUser>>;
    async fn create_user(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> AuthResult<AuthUser>;
    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser>;
    async fn assign_roles(&self, user_id: &UserId, roles: &[String]) -> AuthResult<()>;

    async fn find_or_create_federated(
        &self,
        issuer: &str,
        external_sub: &str,
        claims: &FederatedIdentityClaims,
    ) -> AuthResult<UserId>;

    async fn promote_anonymous(&self, source: &UserId, target: &UserId) -> AuthResult<u64>;
}

pub trait RoleProvider: Send + Sync {
    fn get_roles(&self, user_id: &UserId) -> impl Future<Output = AuthResult<Vec<String>>> + Send;
    fn assign_role(
        &self,
        user_id: &UserId,
        role: &str,
    ) -> impl Future<Output = AuthResult<()>> + Send;
    fn revoke_role(
        &self,
        user_id: &UserId,
        role: &str,
    ) -> impl Future<Output = AuthResult<()>> + Send;
    fn list_users_by_role(
        &self,
        role: &str,
    ) -> impl Future<Output = AuthResult<Vec<AuthUser>>> + Send;
}

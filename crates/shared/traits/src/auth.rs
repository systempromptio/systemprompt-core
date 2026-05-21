//! Authentication and role-management provider traits.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::UserId;

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
    Internal(String),
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
    /// Whether the upstream `IdP` has asserted `email_verified=true` for this
    /// subject. When `false`, callers must refuse to link the federated
    /// identity to a local account that owns the same email — a hostile
    /// upstream could otherwise claim arbitrary accounts.
    pub email_verified: bool,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    pub roles: Vec<String>,
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

    /// Resolve an externally-issued identity (`issuer`, `external_sub`) to a
    /// stable local `UserId`. On first touch creates a new `users` row plus a
    /// `federated_identities` mapping; subsequent calls advance `last_seen_at`
    /// and return the existing id. Implementations MUST perform both writes
    /// in a single transaction.
    async fn find_or_create_federated(
        &self,
        issuer: &str,
        external_sub: &str,
        claims: &FederatedIdentityClaims,
    ) -> AuthResult<UserId>;
}

#[async_trait]
pub trait RoleProvider: Send + Sync {
    async fn get_roles(&self, user_id: &UserId) -> AuthResult<Vec<String>>;
    async fn assign_role(&self, user_id: &UserId, role: &str) -> AuthResult<()>;
    async fn revoke_role(&self, user_id: &UserId, role: &str) -> AuthResult<()>;
    async fn list_users_by_role(&self, role: &str) -> AuthResult<Vec<AuthUser>>;
}

pub type DynUserProvider = Arc<dyn UserProvider>;
pub type DynRoleProvider = Arc<dyn RoleProvider>;

//! Authentication and role-management provider traits.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::UserId;

/// Result alias for [`UserProvider`] / [`RoleProvider`] operations.
pub type AuthResult<T> = Result<T, AuthProviderError>;

/// Errors returned by authentication providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AuthProviderError {
    /// The supplied credentials were rejected.
    #[error("Invalid credentials")]
    InvalidCredentials,

    /// No user matched the lookup key.
    #[error("User not found")]
    UserNotFound,

    /// The token presented for verification could not be parsed.
    #[error("Invalid token")]
    InvalidToken,

    /// The presented token has expired.
    #[error("Token expired")]
    TokenExpired,

    /// The user lacks the permissions required to perform the action.
    #[error("Insufficient permissions")]
    InsufficientPermissions,

    /// Catch-all for unexpected failures inside the provider.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for AuthProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Authenticated user payload returned by [`UserProvider`].
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// Stable user identifier.
    pub id: UserId,
    /// Login name.
    pub name: String,
    /// Primary email address.
    pub email: String,
    /// Roles currently assigned to the user.
    pub roles: Vec<String>,
    /// Whether the account is enabled.
    pub is_active: bool,
}

/// Read/write user-account operations.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn UserProvider>` via [`DynUserProvider`].
#[async_trait]
pub trait UserProvider: Send + Sync {
    /// Look up a user by id.
    async fn find_by_id(&self, id: &UserId) -> AuthResult<Option<AuthUser>>;
    /// Look up a user by email address.
    async fn find_by_email(&self, email: &str) -> AuthResult<Option<AuthUser>>;
    /// Look up a user by login name.
    async fn find_by_name(&self, name: &str) -> AuthResult<Option<AuthUser>>;
    /// Create a new account.
    async fn create_user(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> AuthResult<AuthUser>;
    /// Create an anonymous account keyed by browser fingerprint.
    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser>;
    /// Replace the role set for `user_id`.
    async fn assign_roles(&self, user_id: &UserId, roles: &[String]) -> AuthResult<()>;
}

/// Role lookup and assignment.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn RoleProvider>` via [`DynRoleProvider`].
#[async_trait]
pub trait RoleProvider: Send + Sync {
    /// Return the roles associated with `user_id`.
    async fn get_roles(&self, user_id: &UserId) -> AuthResult<Vec<String>>;
    /// Grant `role` to `user_id`.
    async fn assign_role(&self, user_id: &UserId, role: &str) -> AuthResult<()>;
    /// Revoke `role` from `user_id`.
    async fn revoke_role(&self, user_id: &UserId, role: &str) -> AuthResult<()>;
    /// List every user currently holding `role`.
    async fn list_users_by_role(&self, role: &str) -> AuthResult<Vec<AuthUser>>;
}

/// Shared `Arc` alias for [`UserProvider`].
pub type DynUserProvider = Arc<dyn UserProvider>;
/// Shared `Arc` alias for [`RoleProvider`].
pub type DynRoleProvider = Arc<dyn RoleProvider>;

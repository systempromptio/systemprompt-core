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

impl From<anyhow::Error> for AuthProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
    pub is_active: bool,
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

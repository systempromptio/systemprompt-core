use async_trait::async_trait;
use std::sync::Arc;

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
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
    pub token_type: String,
}

impl TokenPair {
    #[must_use]
    pub fn new(access_token: String, refresh_token: Option<String>, expires_in: i64) -> Self {
        Self {
            access_token,
            refresh_token,
            expires_in,
            token_type: "Bearer".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenClaims {
    pub subject: String,
    pub username: String,
    pub email: Option<String>,
    pub audiences: Vec<String>,
    pub permissions: Vec<String>,
    pub expires_at: i64,
    pub issued_at: i64,
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn validate_token(&self, token: &str) -> AuthResult<TokenClaims>;
    async fn refresh_token(&self, refresh_token: &str) -> AuthResult<TokenPair>;
    async fn revoke_token(&self, token: &str) -> AuthResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthAction {
    Read,
    Write,
    Delete,
    Admin,
    Custom(String),
}

impl AuthAction {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Delete => "delete",
            Self::Admin => "admin",
            Self::Custom(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthPermission {
    pub resource: String,
    pub action: AuthAction,
}

impl AuthPermission {
    #[must_use]
    pub fn new(resource: impl Into<String>, action: AuthAction) -> Self {
        Self {
            resource: resource.into(),
            action,
        }
    }
}

#[async_trait]
pub trait AuthorizationProvider: Send + Sync {
    async fn authorize(
        &self,
        user_id: &str,
        resource: &str,
        action: &AuthAction,
    ) -> AuthResult<bool>;
    async fn get_permissions(&self, user_id: &str) -> AuthResult<Vec<AuthPermission>>;
    async fn has_audience(&self, token: &str, audience: &str) -> AuthResult<bool>;
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
    pub is_active: bool,
}

#[async_trait]
pub trait UserProvider: Send + Sync {
    async fn find_by_id(&self, id: &str) -> AuthResult<Option<AuthUser>>;
    async fn find_by_email(&self, email: &str) -> AuthResult<Option<AuthUser>>;
    async fn find_by_name(&self, name: &str) -> AuthResult<Option<AuthUser>>;
    async fn create_user(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> AuthResult<AuthUser>;
    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser>;
    async fn assign_roles(&self, user_id: &str, roles: &[String]) -> AuthResult<()>;
}

#[async_trait]
pub trait RoleProvider: Send + Sync {
    async fn get_roles(&self, user_id: &str) -> AuthResult<Vec<String>>;
    async fn assign_role(&self, user_id: &str, role: &str) -> AuthResult<()>;
    async fn revoke_role(&self, user_id: &str, role: &str) -> AuthResult<()>;
    async fn list_users_by_role(&self, role: &str) -> AuthResult<Vec<AuthUser>>;
}

pub type DynAuthProvider = Arc<dyn AuthProvider>;
pub type DynAuthorizationProvider = Arc<dyn AuthorizationProvider>;
pub type DynUserProvider = Arc<dyn UserProvider>;
pub type DynRoleProvider = Arc<dyn RoleProvider>;

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_models::auth::JwtAudience;
use systemprompt_traits::{
    AuthAction, AuthPermission, AuthProvider, AuthProviderError, AuthResult, AuthorizationProvider,
    TokenClaims, TokenPair,
};

use crate::models::JwtClaims as OAuthJwtClaims;
use crate::services::validation::jwt as jwt_validation;

#[derive(Debug, Clone)]
pub struct JwtAuthProvider {
    secret: String,
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl JwtAuthProvider {
    pub const fn new(secret: String, issuer: String, audiences: Vec<JwtAudience>) -> Self {
        Self {
            secret,
            issuer,
            audiences,
        }
    }

    pub fn from_context(_context: &systemprompt_runtime::AppContext) -> anyhow::Result<Self> {
        let config = systemprompt_models::Config::get()?;
        Ok(Self {
            secret: systemprompt_models::SecretsBootstrap::jwt_secret()?.to_string(),
            issuer: config.jwt_issuer.clone(),
            audiences: config.jwt_audiences.clone(),
        })
    }
}

fn convert_claims(claims: OAuthJwtClaims) -> TokenClaims {
    TokenClaims {
        subject: claims.sub,
        username: claims.username,
        email: Some(claims.email),
        audiences: claims.aud.iter().map(ToString::to_string).collect(),
        permissions: claims.scope.iter().map(ToString::to_string).collect(),
        expires_at: claims.exp,
        issued_at: claims.iat,
    }
}

#[async_trait]
impl AuthProvider for JwtAuthProvider {
    async fn validate_token(&self, token: &str) -> AuthResult<TokenClaims> {
        let claims =
            jwt_validation::validate_jwt_token(token, &self.secret, &self.issuer, &self.audiences)
                .map_err(|e| {
                    AuthProviderError::Internal(format!("Token validation failed: {e}"))
                })?;

        Ok(convert_claims(claims))
    }

    async fn refresh_token(&self, _refresh_token: &str) -> AuthResult<TokenPair> {
        Err(AuthProviderError::Internal(
            "Token refresh not yet implemented via trait".to_string(),
        ))
    }

    async fn revoke_token(&self, _token: &str) -> AuthResult<()> {
        Err(AuthProviderError::Internal(
            "Token revocation not yet implemented via trait".to_string(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct JwtAuthorizationProvider {
    secret: String,
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl JwtAuthorizationProvider {
    pub const fn new(secret: String, issuer: String, audiences: Vec<JwtAudience>) -> Self {
        Self {
            secret,
            issuer,
            audiences,
        }
    }

    pub fn from_context(_context: &systemprompt_runtime::AppContext) -> anyhow::Result<Self> {
        let config = systemprompt_models::Config::get()?;
        Ok(Self {
            secret: systemprompt_models::SecretsBootstrap::jwt_secret()?.to_string(),
            issuer: config.jwt_issuer.clone(),
            audiences: config.jwt_audiences.clone(),
        })
    }
}

#[async_trait]
impl AuthorizationProvider for JwtAuthorizationProvider {
    async fn authorize(
        &self,
        _user_id: &str,
        _resource: &str,
        _action: &AuthAction,
    ) -> AuthResult<bool> {
        Ok(true)
    }

    async fn get_permissions(&self, _user_id: &str) -> AuthResult<Vec<AuthPermission>> {
        Ok(vec![])
    }

    async fn has_audience(&self, token: &str, audience: &str) -> AuthResult<bool> {
        let claims =
            jwt_validation::validate_jwt_token(token, &self.secret, &self.issuer, &self.audiences)
                .map_err(|e| {
                    AuthProviderError::Internal(format!("Token validation failed: {e}"))
                })?;

        let has_aud = claims.aud.iter().any(|a| a.to_string() == audience);
        Ok(has_aud)
    }
}

#[derive(Clone)]
pub struct TraitBasedAuthService {
    auth_provider: Arc<dyn AuthProvider>,
    authorization_provider: Arc<dyn AuthorizationProvider>,
}

impl std::fmt::Debug for TraitBasedAuthService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TraitBasedAuthService")
            .field("auth_provider", &"AuthProvider")
            .field("authorization_provider", &"AuthorizationProvider")
            .finish()
    }
}

impl TraitBasedAuthService {
    pub fn new(
        auth_provider: Arc<dyn AuthProvider>,
        authorization_provider: Arc<dyn AuthorizationProvider>,
    ) -> Self {
        Self {
            auth_provider,
            authorization_provider,
        }
    }

    pub fn from_config() -> anyhow::Result<Self> {
        let config = systemprompt_models::Config::get()?;
        let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()?.to_string();
        let auth = Arc::new(JwtAuthProvider::new(
            jwt_secret.clone(),
            config.jwt_issuer.clone(),
            config.jwt_audiences.clone(),
        ));
        let authz = Arc::new(JwtAuthorizationProvider::new(
            jwt_secret,
            config.jwt_issuer.clone(),
            config.jwt_audiences.clone(),
        ));
        Ok(Self::new(auth, authz))
    }

    pub fn auth_provider(&self) -> &Arc<dyn AuthProvider> {
        &self.auth_provider
    }

    pub fn authorization_provider(&self) -> &Arc<dyn AuthorizationProvider> {
        &self.authorization_provider
    }

    pub async fn validate_token(&self, token: &str) -> AuthResult<TokenClaims> {
        self.auth_provider.validate_token(token).await
    }

    pub async fn has_audience(&self, token: &str, audience: &str) -> AuthResult<bool> {
        self.authorization_provider.has_audience(token, audience).await
    }
}

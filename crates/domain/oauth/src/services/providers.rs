use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission};
use systemprompt_traits::{
    AgentJwtClaims, GenerateTokenParams, JwtProviderError, JwtResult, JwtValidationProvider,
};
use uuid::Uuid;

use super::generation::{generate_jwt, generate_secure_token, JwtConfig, JwtSigningParams};
use super::validation::jwt::validate_jwt_token;

#[derive(Debug)]
pub struct JwtValidationProviderImpl {
    secret: String,
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl JwtValidationProviderImpl {
    #[must_use]
    pub const fn new(secret: String, issuer: String, audiences: Vec<JwtAudience>) -> Self {
        Self {
            secret,
            issuer,
            audiences,
        }
    }

    pub fn from_config() -> JwtResult<Self> {
        let secret = systemprompt_models::SecretsBootstrap::jwt_secret()
            .map_err(|e| JwtProviderError::ConfigurationError(e.to_string()))?;
        let config = systemprompt_models::Config::get()
            .map_err(|e| JwtProviderError::ConfigurationError(e.to_string()))?;

        Ok(Self {
            secret: secret.to_string(),
            issuer: config.jwt_issuer.clone(),
            audiences: config.jwt_audiences.clone(),
        })
    }
}

impl JwtValidationProvider for JwtValidationProviderImpl {
    fn validate_token(&self, token: &str) -> JwtResult<AgentJwtClaims> {
        let claims =
            validate_jwt_token(token, &self.secret, &self.issuer, &self.audiences)
                .map_err(|e| {
                    if e.to_string().contains("expired") {
                        JwtProviderError::TokenExpired
                    } else {
                        JwtProviderError::InvalidToken
                    }
                })?;

        let is_admin = claims.is_admin();
        Ok(AgentJwtClaims {
            subject: claims.sub,
            username: claims.username,
            user_type: claims.user_type.to_string(),
            audiences: claims.aud.iter().map(ToString::to_string).collect(),
            permissions: claims.scope.iter().map(ToString::to_string).collect(),
            is_admin,
            expires_at: claims.exp,
            issued_at: claims.iat,
        })
    }

    fn generate_token(&self, params: GenerateTokenParams) -> JwtResult<String> {
        let user_id = Uuid::parse_str(&params.user_id)
            .unwrap_or_else(|_| Uuid::new_v4());

        let user = AuthenticatedUser {
            id: user_id,
            username: params.username.clone(),
            email: params.username.clone(),
            roles: vec![],
            permissions: vec![],
        };

        let permissions: Vec<Permission> = params
            .permissions
            .iter()
            .filter_map(|p| p.parse().ok())
            .collect();

        let audiences: Vec<JwtAudience> = params
            .audiences
            .iter()
            .filter_map(|a| a.parse().ok())
            .collect();

        let config = JwtConfig {
            permissions,
            audience: if audiences.is_empty() {
                JwtAudience::standard()
            } else {
                audiences
            },
            expires_in_hours: params.expires_in_hours.map(i64::from),
        };

        let jti = generate_secure_token("jwt");
        let signing = JwtSigningParams {
            secret: &self.secret,
            issuer: &self.issuer,
        };

        generate_jwt(&user, config, jti, &params.session_id, &signing)
            .map_err(|e| JwtProviderError::Internal(e.to_string()))
    }

    fn generate_secure_token(&self, prefix: &str) -> String {
        generate_secure_token(prefix)
    }
}

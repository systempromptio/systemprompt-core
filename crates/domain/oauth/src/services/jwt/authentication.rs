use crate::services::validation::jwt as jwt_validation;
use anyhow::Result;
use axum::http::{HeaderMap, StatusCode};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_security::TokenExtractor;
use uuid::Uuid;

#[derive(Debug, Copy, Clone)]
pub struct AuthenticationService;

impl AuthenticationService {
    pub async fn authenticate(headers: &HeaderMap) -> Result<AuthenticatedUser, StatusCode> {
        let token = TokenExtractor::standard()
            .extract(headers)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let config =
            systemprompt_models::Config::get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let claims = jwt_validation::validate_jwt_token(
            &token,
            jwt_secret,
            &config.jwt_issuer,
            &config.jwt_audiences,
        )
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| StatusCode::UNAUTHORIZED)?;
        let permissions = claims.get_permissions();
        let roles = claims.roles().to_vec();

        Ok(AuthenticatedUser::new_with_roles(
            user_id,
            claims.username.clone(),
            claims.email,
            permissions,
            roles,
        ))
    }
}

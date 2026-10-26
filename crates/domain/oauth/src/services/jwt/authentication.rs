//! JWT-based authentication service entry point.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::validation::jwt as jwt_validation;
use http::{HeaderMap, StatusCode};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::Config;
use systemprompt_models::auth::{AuthenticatedUser, TokenAuthority};
use systemprompt_security::TokenExtractor;

#[derive(Debug, Copy, Clone)]
pub struct AuthenticationService;

impl AuthenticationService {
    pub fn authenticate(
        authority: &dyn TokenAuthority,
        config: &Config,
        headers: &HeaderMap,
    ) -> Result<AuthenticatedUser, StatusCode> {
        let token = TokenExtractor::standard()
            .extract(headers)
            .map_err(|_e| StatusCode::UNAUTHORIZED)?;
        let claims = jwt_validation::validate_jwt_token(authority, &token, &config.jwt_audiences)
            .map_err(|_e| StatusCode::UNAUTHORIZED)?;

        let user_id = UserId::try_new(claims.registered.sub.as_str())
            .map_err(|_e| StatusCode::UNAUTHORIZED)?;
        let permissions = claims.get_permissions();
        let roles = claims.roles().to_vec();

        Ok(AuthenticatedUser::new_with_roles(
            user_id,
            claims.custom.username.clone(),
            claims.custom.email,
            permissions,
            roles,
        ))
    }
}

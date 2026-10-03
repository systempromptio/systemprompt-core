//! Access- and refresh-token minting for a user-bound grant
//! (`authorization_code` and `refresh_token`).
//!
//! Resolves the granted permissions against the user's own, binds an OAuth
//! session, signs the access token and stores the rotated refresh token,
//! carrying the refresh-token family forward when one is given.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{
    AccessTokenId, ClientId, RefreshTokenId, SessionId, SessionSource, UserId,
};
use systemprompt_manifest::Config;
use systemprompt_models::auth::{
    AuthenticatedUser, Permission, parse_permissions, permissions_to_string,
};
use systemprompt_models::errors::{GlobalConfigError, ParseEnumError};
use systemprompt_oauth::repository::{OAuthRepository, RefreshTokenParams};
use systemprompt_oauth::services::{
    JwtConfig, JwtSigningParams, SessionCreationError, SessionCreationService, generate_jwt,
    generate_secure_token, load_authenticated_user,
};
use systemprompt_oauth::{OAuthState, OauthError};
use systemprompt_traits::ExtractSignals;
use thiserror::Error;

use crate::{RequestOrigin, TokenResponse};

/// Failure minting a user-bound token pair.
#[derive(Debug, Error)]
pub enum UserTokenError {
    #[error("Configuration unavailable")]
    Config(#[from] GlobalConfigError),
    #[error("Scope is required for token generation")]
    MissingScope,
    #[error("Requested scope is not a list of known permissions")]
    UnparseableScope(#[from] ParseEnumError),
    #[error("No valid permissions available for user")]
    NoPermissions,
    #[error("Failed to create session")]
    Session(#[from] SessionCreationError),
    #[error(transparent)]
    Oauth(#[from] OauthError),
}

#[derive(Debug)]
pub struct UserTokenParams<'a> {
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub scope: Option<&'a str>,
    pub origin: RequestOrigin<'a>,
    pub resource: Option<&'a str>,
    pub family_id: Option<&'a str>,
}

#[derive(Debug)]
pub struct GeneratedTokens {
    pub response: TokenResponse,
    pub refresh_token_id: RefreshTokenId,
}

pub async fn generate_tokens_by_user_id(
    repo: &OAuthRepository,
    params: UserTokenParams<'_>,
    state: &OAuthState,
) -> Result<GeneratedTokens, UserTokenError> {
    let expires_in = Config::get()?.jwt_access_token_expiration;

    let scope_str = params.scope.ok_or(UserTokenError::MissingScope)?;

    let user = load_authenticated_user(state.user_provider().as_ref(), params.user_id).await?;

    let requested_permissions = parse_permissions(scope_str)?;
    let final_permissions = resolve_user_permissions(&requested_permissions, user.permissions())?;
    let session_service = SessionCreationService::new(
        Arc::clone(state.session_provider()),
        Arc::clone(state.user_provider()),
    );
    let analytics = state.analytics_provider().extract_analytics(
        params.origin.headers,
        ExtractSignals {
            caller_ip: params.origin.caller_ip,
            ..Default::default()
        },
    );
    let session_id = session_service
        .create_authenticated_session(params.user_id, &analytics, SessionSource::Oauth)
        .await?;

    let jwt_and_refresh =
        create_jwt_and_refresh_token(repo, &user, final_permissions, &session_id, &params).await?;

    if let Err(e) = repo.update_client_last_used(params.client_id).await {
        tracing::warn!(
            client_id = %params.client_id,
            error = %e,
            "Failed to update client last_used timestamp"
        );
    }

    Ok(GeneratedTokens {
        response: TokenResponse {
            access_token: jwt_and_refresh.access_token,
            token_type: "Bearer".to_owned(),
            expires_in,
            refresh_token: Some(jwt_and_refresh.refresh_token_value),
            scope: Some(jwt_and_refresh.scope_string),
            issued_token_type: None,
        },
        refresh_token_id: jwt_and_refresh.refresh_token_id,
    })
}

struct JwtAndRefreshToken {
    access_token: String,
    refresh_token_value: String,
    scope_string: String,
    refresh_token_id: RefreshTokenId,
}

async fn create_jwt_and_refresh_token(
    repo: &OAuthRepository,
    user: &AuthenticatedUser,
    permissions: Vec<Permission>,
    session_id: &SessionId,
    params: &UserTokenParams<'_>,
) -> Result<JwtAndRefreshToken, UserTokenError> {
    let scope_string = permissions_to_string(&permissions);
    let access_token_jti = AccessTokenId::generate();
    let global_config = Config::get()?;
    let config = JwtConfig {
        permissions,
        audience: global_config.jwt_audiences.clone(),
        resource: params.resource.map(String::from),
        expires_in: chrono::Duration::seconds(global_config.jwt_access_token_expiration),
        plugin_id: None,
        client_id: Some(params.client_id.clone()),
    };
    let signing = JwtSigningParams {
        issuer: &global_config.jwt_issuer,
    };
    let access_token = generate_jwt(user, config, access_token_jti, session_id, &signing)?;

    let refresh_token_value = generate_secure_token("rt");
    let refresh_token_id = RefreshTokenId::new(&refresh_token_value);
    let refresh_expires_at =
        chrono::Utc::now().timestamp() + global_config.jwt_refresh_token_expiration;

    let mut builder = RefreshTokenParams::builder(
        &refresh_token_id,
        params.client_id,
        params.user_id,
        &scope_string,
        refresh_expires_at,
    );
    if let Some(family) = params.family_id {
        builder = builder.with_family(family);
    }
    repo.store_refresh_token(builder.build()).await?;

    Ok(JwtAndRefreshToken {
        access_token,
        refresh_token_value,
        scope_string,
        refresh_token_id,
    })
}

pub fn resolve_user_permissions(
    requested_permissions: &[Permission],
    user_permissions: &[Permission],
) -> Result<Vec<Permission>, UserTokenError> {
    let mut final_permissions = Vec::new();

    for requested in requested_permissions {
        if *requested == Permission::User {
            final_permissions.extend(
                user_permissions
                    .iter()
                    .filter(|p| p.is_user_role())
                    .copied(),
            );
        } else if user_permissions.contains(requested) {
            final_permissions.push(*requested);
        }
    }

    final_permissions.sort_by_key(|p| std::cmp::Reverse(p.hierarchy_level()));
    final_permissions.dedup();

    if final_permissions.is_empty() {
        return Err(UserTokenError::NoPermissions);
    }

    Ok(final_permissions)
}

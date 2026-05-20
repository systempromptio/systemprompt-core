mod client_credentials;

pub use client_credentials::{ClientTokenOptions, generate_client_tokens};

use super::{TokenError, TokenErrorResponse, TokenResponse, TokenResult};
use anyhow::Result;
use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use std::sync::Arc;
use systemprompt_identifiers::{ClientId, RefreshTokenId, SessionId, SessionSource, UserId};
use systemprompt_models::Config;
use systemprompt_models::auth::{AuthenticatedUser, Permission, parse_permissions};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::{OAuthRepository, RefreshTokenParams};
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt};

#[derive(Debug)]
pub struct TokenGenerationParams<'a> {
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub scope: Option<&'a str>,
    pub headers: &'a HeaderMap,
    pub resource: Option<&'a str>,
    /// If `Some`, the new refresh token joins this existing family (rotation).
    /// `None` mints a fresh family rooted at the new token id (initial
    /// authorization-code exchange).
    pub family_id: Option<&'a str>,
}

#[derive(Debug)]
pub struct GeneratedTokens {
    pub response: TokenResponse,
    pub refresh_token_id: String,
}

pub async fn generate_tokens_by_user_id(
    repo: &OAuthRepository,
    params: TokenGenerationParams<'_>,
    state: &OAuthState,
) -> Result<GeneratedTokens> {
    let expires_in = Config::get()?.jwt_access_token_expiration;

    let scope_str = params
        .scope
        .ok_or_else(|| anyhow::anyhow!("Scope is required for token generation"))?;

    let user = load_authenticated_user(repo, params.user_id).await?;

    let requested_permissions = parse_permissions(scope_str)?;
    let user_perms = user.permissions().to_vec();
    let final_permissions = resolve_user_permissions(&requested_permissions, &user_perms)?;
    let session_service = systemprompt_oauth::services::SessionCreationService::new(
        Arc::clone(state.analytics_provider()),
        Arc::clone(state.user_provider()),
    );
    let session_id = session_service
        .create_authenticated_session(params.user_id, params.headers, SessionSource::Oauth)
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
            token_type: "Bearer".to_string(),
            expires_in,
            refresh_token: Some(jwt_and_refresh.refresh_token_value),
            scope: Some(jwt_and_refresh.scope_string),
        },
        refresh_token_id: jwt_and_refresh.refresh_token_id,
    })
}

pub async fn load_authenticated_user(
    repo: &OAuthRepository,
    user_id: &UserId,
) -> Result<AuthenticatedUser> {
    repo.get_authenticated_user(user_id)
        .await
        .map_err(Into::into)
}

struct JwtAndRefreshToken {
    access_token: String,
    refresh_token_value: String,
    scope_string: String,
    refresh_token_id: String,
}

async fn create_jwt_and_refresh_token(
    repo: &OAuthRepository,
    user: &AuthenticatedUser,
    permissions: Vec<Permission>,
    session_id: &SessionId,
    params: &TokenGenerationParams<'_>,
) -> Result<JwtAndRefreshToken> {
    use systemprompt_oauth::services::{generate_access_token_jti, generate_secure_token};

    let scope_string = systemprompt_models::auth::permissions_to_string(&permissions);
    let access_token_jti = generate_access_token_jti();
    let jwt_secret = systemprompt_config::SecretsBootstrap::jwt_secret()?;
    let global_config = Config::get()?;
    let config = JwtConfig {
        permissions,
        audience: global_config.jwt_audiences.clone(),
        resource: params.resource.map(String::from),
        expires_in_hours: Some(global_config.jwt_access_token_expiration / 3600),
        plugin_id: None,
    };
    let signing = JwtSigningParams {
        secret: jwt_secret,
        issuer: &global_config.jwt_issuer,
    };
    let access_token = generate_jwt(user, config, access_token_jti, session_id, &signing)?;

    let refresh_token_value = generate_secure_token("rt");
    let refresh_token_id = RefreshTokenId::new(&refresh_token_value);
    let refresh_expires_at =
        chrono::Utc::now().timestamp() + Config::get()?.jwt_refresh_token_expiration;

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
        refresh_token_id: refresh_token_id.as_str().to_string(),
    })
}

pub fn resolve_user_permissions(
    requested_permissions: &[Permission],
    user_permissions: &[Permission],
) -> Result<Vec<Permission>> {
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
        return Err(anyhow::anyhow!("No valid permissions available for user"));
    }

    Ok(final_permissions)
}

pub fn convert_token_result_to_response(
    result: TokenResult<TokenResponse>,
) -> axum::response::Response {
    match result {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => {
            let status = match &error {
                TokenError::InvalidClientSecret => StatusCode::UNAUTHORIZED,
                TokenError::ServerError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::BAD_REQUEST,
            };
            let error_response: TokenErrorResponse = error.into();
            (status, Json(error_response)).into_response()
        },
    }
}

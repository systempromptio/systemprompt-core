use super::{TokenError, TokenErrorResponse, TokenResponse, TokenResult};
use crate::repository::{OAuthRepository, RefreshTokenParams};
use crate::services::{generate_jwt, JwtConfig, JwtSigningParams};
use anyhow::Result;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::sync::Arc;
use systemprompt_identifiers::{ClientId, RefreshTokenId, SessionId, SessionSource, UserId};
use systemprompt_models::auth::{parse_permissions, AuthenticatedUser, Permission};
use systemprompt_models::Config;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserProviderImpl, UserService};

pub struct TokenGenerationParams<'a> {
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub scope: Option<&'a str>,
    pub headers: &'a HeaderMap,
}

pub async fn generate_tokens_by_user_id(
    repo: &OAuthRepository,
    params: TokenGenerationParams<'_>,
    ctx: &AppContext,
) -> Result<TokenResponse> {
    use crate::services::{generate_access_token_jti, generate_secure_token};

    let expires_in = Config::get()?.jwt_access_token_expiration;
    let refresh_token_expires_in = Config::get()?.jwt_refresh_token_expiration;

    let scope_str = params
        .scope
        .ok_or_else(|| anyhow::anyhow!("Scope is required for token generation"))?;

    let user = load_authenticated_user(repo, params.user_id).await?;

    let requested_permissions = parse_permissions(scope_str)?;
    let user_perms = user.permissions().to_vec();
    let final_permissions =
        resolve_user_permissions(repo, &requested_permissions, &user_perms, params.client_id)
            .await?;
    let scope_string = systemprompt_models::auth::permissions_to_string(&final_permissions);

    let user_provider = Arc::new(UserProviderImpl::new(UserService::new(ctx.db_pool())?));
    let session_service = crate::services::SessionCreationService::new(
        Arc::clone(ctx.analytics_service()),
        user_provider,
    );
    let session_id = session_service
        .create_authenticated_session(params.user_id, params.headers, SessionSource::Oauth)
        .await?;

    let access_token_jti = generate_access_token_jti();
    let config = JwtConfig {
        permissions: final_permissions,
        ..Default::default()
    };
    let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()?;
    let global_config = Config::get()?;
    let signing = JwtSigningParams {
        secret: jwt_secret,
        issuer: &global_config.jwt_issuer,
    };
    let access_token = generate_jwt(&user, config, access_token_jti, &session_id, &signing)?;

    let refresh_token_value = generate_secure_token("rt");
    let refresh_token_id = RefreshTokenId::new(&refresh_token_value);
    let refresh_expires_at = chrono::Utc::now().timestamp() + refresh_token_expires_in;

    let refresh_params = RefreshTokenParams::builder(
        &refresh_token_id,
        params.client_id,
        params.user_id,
        &scope_string,
        refresh_expires_at,
    )
    .build();
    repo.store_refresh_token(refresh_params).await?;

    if let Err(e) = repo
        .update_client_last_used(params.client_id.as_str())
        .await
    {
        tracing::warn!(
            client_id = %params.client_id,
            error = %e,
            "Failed to update client last_used timestamp"
        );
    }

    Ok(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: Some(refresh_token_value),
        scope: Some(scope_string),
    })
}

pub async fn load_authenticated_user(
    repo: &OAuthRepository,
    user_id: &UserId,
) -> Result<AuthenticatedUser> {
    repo.get_authenticated_user(user_id.as_str()).await
}

pub async fn generate_client_tokens(
    repo: &OAuthRepository,
    client_id: &ClientId,
    scope: Option<&str>,
) -> Result<TokenResponse> {
    let expires_in = Config::get()?.jwt_access_token_expiration;

    let scope_str =
        scope.ok_or_else(|| anyhow::anyhow!("Scope is required for client credentials grant"))?;

    let requested_permissions = parse_permissions(scope_str)?;

    let client = repo
        .find_client_by_id(client_id.as_str())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Client not found"))?;

    let client_allowed: Vec<Permission> = client
        .scopes
        .iter()
        .filter_map(|s| {
            Permission::from_str(s)
                .map_err(|e| {
                    tracing::warn!(scope = %s, error = %e, "Invalid scope in client configuration");
                    e
                })
                .ok()
        })
        .collect();

    let permissions: Vec<Permission> = requested_permissions
        .into_iter()
        .filter(|p| client_allowed.contains(p))
        .collect();

    if permissions.is_empty() {
        return Err(anyhow::anyhow!(
            "No valid permissions: requested scopes not allowed for this client"
        ));
    }

    let client_id_str = client_id.as_str();
    let mut hasher = Sha256::new();
    hasher.update(format!("client.{client_id_str}").as_bytes());
    let hash = hasher.finalize();

    let mut uuid_bytes = [0u8; 16];
    uuid_bytes.copy_from_slice(&hash[..16]);
    let client_uuid = uuid::Uuid::from_bytes(uuid_bytes);

    let role_strings: Vec<String> = permissions.iter().map(ToString::to_string).collect();
    let client_user = AuthenticatedUser::new_with_roles(
        client_uuid,
        format!("client:{client_id_str}"),
        format!("{client_id_str}@client.local"),
        permissions.clone(),
        role_strings,
    );

    let config = JwtConfig {
        permissions: permissions.clone(),
        ..Default::default()
    };
    let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()?;
    let global_config = Config::get()?;
    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));
    let signing = JwtSigningParams {
        secret: jwt_secret,
        issuer: &global_config.jwt_issuer,
    };
    let jwt_token = generate_jwt(
        &client_user,
        config,
        uuid::Uuid::new_v4().to_string(),
        &session_id,
        &signing,
    )?;

    Ok(TokenResponse {
        access_token: jwt_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: None,
        scope: Some(
            permissions
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        ),
    })
}

pub async fn resolve_user_permissions(
    repo: &OAuthRepository,
    requested_permissions: &[Permission],
    user_permissions: &[Permission],
    client_id: &ClientId,
) -> Result<Vec<Permission>> {
    let client = repo
        .find_client_by_id(client_id.as_str())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Client not found"))?;

    let client_allowed: Vec<Permission> = client
        .scopes
        .iter()
        .filter_map(|s| {
            Permission::from_str(s)
                .map_err(|e| {
                    tracing::warn!(scope = %s, error = %e, "Invalid scope in client configuration");
                    e
                })
                .ok()
        })
        .collect();

    let mut final_permissions = Vec::new();

    for requested in requested_permissions {
        if !client_allowed.contains(requested) {
            continue;
        }

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

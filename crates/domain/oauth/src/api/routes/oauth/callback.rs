use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect};
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;
use systemprompt_core_users::{UserProviderImpl, UserService};
use systemprompt_identifiers::{
    AuthorizationCode, ClientId, RefreshTokenId, SessionSource, UserId,
};
use systemprompt_models::auth::{parse_permissions, AuthenticatedUser, Permission};
use systemprompt_models::Config;
use systemprompt_runtime::AppContext;

use crate::repository::{OAuthRepository, RefreshTokenParams};

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

pub async fn handle_callback(
    Query(params): Query<CallbackQuery>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let repo = match OAuthRepository::new(Arc::clone(ctx.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };
    let config = match Config::get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to load config: {e}"),
            )
                .into_response();
        },
    };

    let server_base_url = &config.api_external_url;
    let redirect_uri = format!("{server_base_url}/api/v1/core/oauth/callback");

    let browser_client = match find_browser_client(&repo, &redirect_uri).await {
        Ok(client) => client,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to find OAuth client: {e}"),
            )
                .into_response();
        },
    };

    let code = AuthorizationCode::new(&params.code);
    let client_id = ClientId::new(&browser_client.client_id);
    let token_response = match exchange_code_for_token(
        &repo,
        CodeExchangeParams {
            code: &code,
            client_id: &client_id,
            redirect_uri: &redirect_uri,
            headers: &headers,
        },
        &ctx,
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                format!("Failed to exchange code for token: {e}"),
            )
                .into_response();
        },
    };

    let redirect_destination = params
        .state
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("/");

    let cookie = format!(
        "access_token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=3600",
        token_response.access_token
    );

    let mut response = Redirect::to(redirect_destination).into_response();
    if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, cookie_value);
    }

    response
}

async fn find_browser_client(
    repo: &OAuthRepository,
    redirect_uri: &str,
) -> anyhow::Result<BrowserClient> {
    let clients = repo.list_clients().await?;

    for client in clients {
        if client.redirect_uris.contains(&redirect_uri.to_string())
            && (client.scopes.contains(&"admin".to_string())
                || client.scopes.contains(&"user".to_string()))
        {
            return Ok(BrowserClient {
                client_id: client.client_id,
            });
        }
    }

    Err(anyhow::anyhow!("No suitable browser client found"))
}

struct CodeExchangeParams<'a> {
    code: &'a AuthorizationCode,
    client_id: &'a ClientId,
    redirect_uri: &'a str,
    headers: &'a HeaderMap,
}

async fn exchange_code_for_token(
    repo: &OAuthRepository,
    params: CodeExchangeParams<'_>,
    ctx: &AppContext,
) -> anyhow::Result<TokenResponse> {
    use crate::services::{
        generate_access_token_jti, generate_jwt, generate_secure_token, JwtConfig, JwtSigningParams,
    };

    let (user_id, scope) = repo
        .validate_authorization_code(
            params.code,
            params.client_id,
            Some(params.redirect_uri),
            None,
        )
        .await?;

    let user = load_authenticated_user(&user_id, Arc::clone(ctx.db_pool())).await?;

    let permissions = parse_permissions(&scope)?;

    let user_provider = Arc::new(UserProviderImpl::new(UserService::new(ctx.db_pool())?));
    let session_service = crate::services::SessionCreationService::new(
        Arc::clone(ctx.analytics_service()),
        user_provider,
    );
    let session_id = session_service
        .create_authenticated_session(&user_id, params.headers, SessionSource::Oauth)
        .await?;

    let access_token_jti = generate_access_token_jti();
    let config = JwtConfig {
        permissions: permissions.clone(),
        ..Default::default()
    };
    let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()?;
    let global_config = systemprompt_models::Config::get()?;
    let signing = JwtSigningParams {
        secret: jwt_secret,
        issuer: &global_config.jwt_issuer,
    };
    let access_token = generate_jwt(&user, config, access_token_jti, &session_id, &signing)?;

    let refresh_token_value = generate_secure_token("rt");
    let refresh_token_id = RefreshTokenId::new(&refresh_token_value);
    let refresh_expires_at = chrono::Utc::now().timestamp() + (86400 * 30);

    let refresh_params = RefreshTokenParams::builder(
        &refresh_token_id,
        params.client_id,
        &user_id,
        &scope,
        refresh_expires_at,
    )
    .build();
    repo.store_refresh_token(refresh_params).await?;

    Ok(TokenResponse { access_token })
}

async fn load_authenticated_user(
    user_id: &UserId,
    db_pool: systemprompt_core_database::DbPool,
) -> anyhow::Result<AuthenticatedUser> {
    let user_service = UserService::new(&db_pool)?;

    let user = user_service
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("User not found: {user_id}"))?;

    let permissions: Vec<Permission> = user
        .roles
        .iter()
        .filter_map(|s| Permission::from_str(s).ok())
        .collect();

    let user_uuid = uuid::Uuid::parse_str(user.id.as_ref())
        .map_err(|_| anyhow::anyhow!("Invalid user UUID: {}", user.id))?;

    Ok(AuthenticatedUser::new_with_roles(
        user_uuid,
        user.name,
        user.email,
        permissions,
        user.roles,
    ))
}

#[derive(Debug)]
struct BrowserClient {
    client_id: String,
}

#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: String,
}

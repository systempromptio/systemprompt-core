//! OAuth callback endpoint for the server's own browser client.
//!
//! Exchanges the returned authorization code for tokens, establishes an
//! authenticated session, sets the access-token cookie, and redirects to the
//! origin-validated `return_to` recovered from the consumed state binding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::sync::Arc;
use systemprompt_identifiers::{
    AccessTokenId, AuthorizationCode, ClientId, RefreshTokenId, SessionSource,
};
use systemprompt_manifest::Config;
use systemprompt_models::auth::parse_permissions;

use crate::routes::oauth::extractors::OAuthRepo;
use crate::routes::oauth::{OAuthHttpError, internal};
use crate::services::middleware::client_addr::ClientIp;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::{OAuthRepository, RefreshTokenParams};
use systemprompt_oauth::services::load_authenticated_user;
use systemprompt_traits::ExtractSignals;

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

pub async fn handle_callback(
    Query(params): Query<CallbackQuery>,
    State(state): State<OAuthState>,
    OAuthRepo(repo): OAuthRepo,
    ClientIp(caller_ip): ClientIp,
    headers: HeaderMap,
) -> Result<Response, OAuthHttpError> {
    let config = Config::get()?;

    let server_base_url = &config.api_external_url;
    let redirect_uri = format!("{server_base_url}/api/v1/core/oauth/callback");

    let browser_client = find_browser_client(&repo, &redirect_uri)
        .await
        .map_err(|e| internal::server_error("Failed to find OAuth client", e))?;

    let code = AuthorizationCode::new(&params.code);
    let client_id = ClientId::new(&browser_client.client_id);
    let token_response = exchange_code_for_token(
        &repo,
        CodeExchangeParams {
            caller_ip,
            code: &code,
            client_id: &client_id,
            redirect_uri: &redirect_uri,
            headers: &headers,
        },
        &state,
    )
    .await
    .map_err(|e| {
        internal::rejected(
            OAuthHttpError::invalid_grant("Failed to exchange code for token")
                .with_status(StatusCode::UNAUTHORIZED),
            e,
        )
    })?;

    let redirect_destination = resolve_redirect_destination(&repo, params.state.as_deref()).await?;

    Ok(session_cookie_redirect(
        &token_response.access_token,
        &redirect_destination,
    ))
}

async fn resolve_redirect_destination(
    repo: &OAuthRepository,
    state_token: Option<&str>,
) -> Result<String, OAuthHttpError> {
    let Some(state_token) = state_token.filter(|s| !s.is_empty()) else {
        return Err(OAuthHttpError::invalid_request("Missing state parameter"));
    };
    match repo.consume_state_binding(state_token).await {
        Ok(Some(binding)) => Ok(binding.return_to),
        Ok(None) => {
            tracing::warn!("state binding missing, expired, or already consumed");
            Err(OAuthHttpError::invalid_request("Invalid state parameter"))
        },
        Err(e) => Err(internal::server_error("Failed to validate state", e)),
    }
}

fn session_cookie_redirect(access_token: &str, destination: &str) -> Response {
    let cookie = format!(
        "access_token={access_token}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={}",
        systemprompt_oauth::constants::token::COOKIE_MAX_AGE_SECONDS
    );

    let mut response = Redirect::to(destination).into_response();
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
    let client = repo
        .find_client_by_redirect_uri_with_scope(redirect_uri, &["admin", "user"])
        .await?
        .ok_or_else(|| anyhow::anyhow!("No suitable browser client found"))?;

    Ok(BrowserClient {
        client_id: client.client_id.to_string(),
    })
}

struct CodeExchangeParams<'a> {
    caller_ip: Option<std::net::IpAddr>,
    code: &'a AuthorizationCode,
    client_id: &'a ClientId,
    redirect_uri: &'a str,
    headers: &'a HeaderMap,
}

async fn exchange_code_for_token(
    repo: &OAuthRepository,
    params: CodeExchangeParams<'_>,
    state: &OAuthState,
) -> anyhow::Result<TokenResponse> {
    use systemprompt_oauth::services::{
        JwtConfig, JwtSigningParams, generate_jwt, generate_secure_token,
    };

    let validation_result = repo
        .validate_authorization_code(params.code, params.client_id, params.redirect_uri, "")
        .await?;

    let user =
        load_authenticated_user(state.user_provider().as_ref(), &validation_result.user_id).await?;

    let permissions = parse_permissions(&validation_result.scope)?;

    let session_service = systemprompt_oauth::services::SessionCreationService::new(
        Arc::clone(state.session_provider()),
        Arc::clone(state.user_provider()),
    );
    let analytics = state.analytics_provider().extract_analytics(
        params.headers,
        ExtractSignals {
            caller_ip: params.caller_ip,
            ..Default::default()
        },
    );
    let session_id = session_service
        .create_authenticated_session(&validation_result.user_id, &analytics, SessionSource::Oauth)
        .await?;

    let access_token_jti = AccessTokenId::generate();
    let global_config = Config::get()?;
    let config = JwtConfig {
        permissions: permissions.clone(),
        audience: global_config.jwt_audiences.clone(),
        client_id: Some(params.client_id.clone()),
        ..Default::default()
    };
    let signing = JwtSigningParams {
        issuer: &global_config.jwt_issuer,
    };
    let access_token = generate_jwt(&user, config, access_token_jti, &session_id, &signing)?;

    let refresh_token_value = generate_secure_token("rt");
    let refresh_token_id = RefreshTokenId::new(&refresh_token_value);
    let refresh_expires_at = chrono::Utc::now().timestamp()
        + (systemprompt_oauth::constants::token::SECONDS_PER_DAY
            * systemprompt_oauth::constants::token::REFRESH_TOKEN_EXPIRY_DAYS);

    let refresh_params = RefreshTokenParams::builder(
        &refresh_token_id,
        params.client_id,
        &validation_result.user_id,
        &validation_result.scope,
        refresh_expires_at,
    )
    .build();
    repo.store_refresh_token(refresh_params).await?;

    if let Err(e) = repo
        .link_auth_code_to_refresh_token(params.code, &refresh_token_id)
        .await
    {
        tracing::warn!(error = %e, "Failed to link auth code to refresh token");
    }

    Ok(TokenResponse { access_token })
}

#[derive(Debug)]
struct BrowserClient {
    client_id: String,
}

#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: String,
}

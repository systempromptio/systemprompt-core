//! `/oauth/token` endpoint: dispatches by `grant_type` to the per-grant
//! handlers in `grants` and normalizes token-exchange errors back into the
//! endpoint's `TokenError` wire type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Form, Json};
use systemprompt_models::RequestContext;
use systemprompt_oauth::{GrantType, OAuthState};
use tracing::instrument;

use base64::Engine;

use super::{TokenError, TokenRequest, TokenResult};
use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;
use crate::services::middleware::client_addr::ClientIp;

mod grants;

use axum::http::HeaderMap;
use grants::{
    handle_authorization_code_grant, handle_client_credentials_grant, handle_jwt_bearer_grant,
    handle_refresh_token_grant, handle_token_exchange_grant,
};

#[expect(
    clippy::too_many_arguments,
    reason = "axum handler: each extractor is a separate parameter"
)]
#[instrument(skip(state, _req_ctx, caller_ip, headers, request, repo), fields(grant_type = %request.grant_type))]
pub async fn handle_token(
    Extension(_req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    OAuthRepo(repo): OAuthRepo,
    ClientIp(caller_ip): ClientIp,
    headers: HeaderMap,
    Form(mut request): Form<TokenRequest>,
) -> Result<Response, OAuthHttpError> {
    tracing::info!(grant_type = %request.grant_type, "Token request received");

    apply_basic_client_auth(&headers, &mut request)?;

    let grant_type = request
        .grant_type
        .parse::<GrantType>()
        .map_err(|_unknown| TokenError::UnsupportedGrantType {
            grant_type: request.grant_type.clone(),
        })?;
    let response = match grant_type {
        GrantType::AuthorizationCode => {
            handle_authorization_code_grant(repo, request, &headers, caller_ip, &state).await?
        },
        GrantType::RefreshToken => {
            handle_refresh_token_grant(repo, request, &headers, caller_ip, &state).await?
        },
        GrantType::ClientCredentials => {
            handle_client_credentials_grant(repo, request, &headers, caller_ip, &state).await?
        },
        GrantType::TokenExchange => {
            handle_token_exchange_grant(repo, request, &headers, caller_ip, &state).await?
        },
        GrantType::JwtBearer => {
            handle_jwt_bearer_grant(repo, request, &headers, caller_ip, &state).await?
        },
    };
    Ok((StatusCode::OK, Json(response)).into_response())
}

// Why: RFC 6749 §2.3.1 — `client_secret_basic` carries
// `client_id:client_secret` percent-encoded inside HTTP Basic; a client must
// not also send them in the body, so a conflicting pair is rejected rather than
// silently preferred.
fn apply_basic_client_auth(headers: &HeaderMap, request: &mut TokenRequest) -> TokenResult<()> {
    let Some(encoded) = headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
    else {
        return Ok(());
    };

    let invalid = || TokenError::InvalidRequest {
        field: "authorization".to_owned(),
        message: "malformed HTTP Basic client credentials".to_owned(),
    };
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|_e| invalid())?;
    let decoded = String::from_utf8(decoded).map_err(|_e| invalid())?;
    let (client_id, client_secret) = decoded.split_once(':').ok_or_else(invalid)?;
    let client_id = urlencoding::decode(client_id).map_err(|_e| invalid())?;
    let client_secret = urlencoding::decode(client_secret).map_err(|_e| invalid())?;

    if request
        .client_id
        .as_deref()
        .is_some_and(|body_id| body_id != client_id)
        || request.client_secret.is_some()
    {
        return Err(TokenError::InvalidRequest {
            field: "client_id".to_owned(),
            message: "client credentials supplied both in the Authorization header and the body"
                .to_owned(),
        });
    }

    request.client_id = Some(client_id.into_owned());
    request.client_secret = Some(client_secret.into_owned());
    Ok(())
}

pub fn map_exchange_error(err: &anyhow::Error) -> TokenError {
    if let Some(token_err) = err.downcast_ref::<TokenError>() {
        return clone_token_error(token_err);
    }
    TokenError::ServerError {
        message: err.to_string(),
    }
}

fn clone_token_error(err: &TokenError) -> TokenError {
    match err {
        TokenError::InvalidRequest { field, message } => TokenError::InvalidRequest {
            field: field.clone(),
            message: message.clone(),
        },
        TokenError::UnsupportedGrantType { grant_type } => TokenError::UnsupportedGrantType {
            grant_type: grant_type.clone(),
        },
        TokenError::InvalidClient => TokenError::InvalidClient,
        TokenError::InvalidGrant { reason } => TokenError::InvalidGrant {
            reason: reason.clone(),
        },
        TokenError::InvalidRefreshToken { reason } => TokenError::InvalidRefreshToken {
            reason: reason.clone(),
        },
        TokenError::InvalidCredentials => TokenError::InvalidCredentials,
        TokenError::InvalidClientSecret => TokenError::InvalidClientSecret,
        TokenError::ExpiredCode => TokenError::ExpiredCode,
        TokenError::ServerError { message } => TokenError::ServerError {
            message: message.clone(),
        },
        TokenError::InvalidTarget { message } => TokenError::InvalidTarget {
            message: message.clone(),
        },
        TokenError::InvalidScope { message } => TokenError::InvalidScope {
            message: message.clone(),
        },
    }
}

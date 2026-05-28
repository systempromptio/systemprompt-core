//! `/oauth/token` endpoint: dispatches by `grant_type` to the per-grant
//! handlers in [`grants`] and normalizes token-exchange errors back into the
//! endpoint's `TokenError` wire type.

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Form, Json};
use systemprompt_models::RequestContext;
use systemprompt_oauth::{GrantType, OAuthState};
use tracing::instrument;

use super::{TokenError, TokenRequest};
use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

mod grants;

use axum::http::HeaderMap;
use grants::{
    handle_authorization_code_grant, handle_client_credentials_grant, handle_refresh_token_grant,
    handle_token_exchange_grant,
};

#[instrument(skip(state, _req_ctx, headers, request, repo), fields(grant_type = %request.grant_type))]
pub async fn handle_token(
    Extension(_req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    OAuthRepo(repo): OAuthRepo,
    headers: HeaderMap,
    Form(request): Form<TokenRequest>,
) -> Result<Response, OAuthHttpError> {
    tracing::info!(grant_type = %request.grant_type, "Token request received");

    let parsed = request.grant_type.parse::<GrantType>().ok();
    let response = match parsed {
        Some(GrantType::AuthorizationCode) => {
            handle_authorization_code_grant(repo, request, &headers, &state).await?
        },
        Some(GrantType::RefreshToken) => {
            handle_refresh_token_grant(repo, request, &headers, &state).await?
        },
        Some(GrantType::ClientCredentials) => {
            handle_client_credentials_grant(repo, request, &headers, &state).await?
        },
        Some(GrantType::TokenExchange) => {
            handle_token_exchange_grant(repo, request, &headers, &state).await?
        },
        None => {
            return Err(TokenError::UnsupportedGrantType {
                grant_type: request.grant_type.clone(),
            }
            .into());
        },
    };
    Ok((StatusCode::OK, Json(response)).into_response())
}

pub(super) fn map_exchange_error(err: &anyhow::Error) -> TokenError {
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

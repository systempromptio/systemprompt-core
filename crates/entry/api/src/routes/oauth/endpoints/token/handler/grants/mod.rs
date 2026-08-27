//! Per-`grant_type` token issuance: authorization-code, refresh-token,
//! client-credentials, RFC 8693 token-exchange, and the RFC 7523 jwt-bearer
//! assertion grant that redeems an ID-JAG.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use std::net::IpAddr;
use systemprompt_identifiers::{AuthorizationCode, ClientId, RefreshTokenId};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;

use super::super::generation::{TokenGenerationParams, generate_tokens_by_user_id};
use super::super::validation::{
    AuthCodeValidationParams, extract_required_field, validate_authorization_code,
    validate_client_credentials,
};
use super::super::{TokenError, TokenRequest, TokenResponse};


mod exchange;

pub(super) use self::exchange::{
    handle_client_credentials_grant, handle_jwt_bearer_grant, handle_token_exchange_grant,
};

pub(super) async fn handle_authorization_code_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    caller_ip: Option<IpAddr>,
    state: &OAuthState,
) -> Result<TokenResponse, TokenError> {
    let code_str = extract_required_field(request.code.as_deref(), "code")?;
    let code = AuthorizationCode::new(code_str);

    let client_id = if let Some(id) = request.client_id.as_deref() {
        ClientId::new(id)
    } else {
        repo.find_client_id_from_auth_code(&code)
            .await
            .map_err(|e| TokenError::ServerError {
                message: format!("Failed to lookup authorization code: {e}"),
            })?
            .ok_or_else(|| TokenError::InvalidGrant {
                reason: "Invalid or expired authorization code".to_owned(),
            })?
    };

    validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
        .await
        .map_err(|_e| TokenError::InvalidClientSecret)?;

    let validation_result = validate_authorization_code(AuthCodeValidationParams {
        repo: &repo,
        code: &code,
        client_id: &client_id,
        redirect_uri: request.redirect_uri.as_deref(),
        code_verifier: request.code_verifier.as_deref(),
        request_resource: request.resource.as_deref(),
    })
    .await
    .map_err(|e: anyhow::Error| TokenError::InvalidGrant {
        reason: e.to_string(),
    })?;

    let generated = generate_tokens_by_user_id(
        &repo,
        TokenGenerationParams {
            client_id: &client_id,
            user_id: &validation_result.user_id,
            scope: Some(&validation_result.scope),
            headers,
            caller_ip,
            resource: validation_result.resource.as_deref(),
            family_id: None,
        },
        state,
    )
    .await
    .map_err(|e| TokenError::ServerError {
        message: e.to_string(),
    })?;

    if let Err(e) = repo
        .link_auth_code_to_refresh_token(&code, &generated.refresh_token_id)
        .await
    {
        tracing::warn!(error = %e, "Failed to link auth code to refresh token");
    }

    let token_response = generated.response;
    tracing::info!(
        grant_type = "authorization_code",
        client_id = %client_id,
        user_id = %validation_result.user_id,
        scope = %validation_result.scope,
        resource = ?validation_result.resource,
        token_type = %token_response.token_type,
        expires_in = token_response.expires_in,
        "Token issued"
    );

    Ok(token_response)
}

pub(super) async fn handle_refresh_token_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    caller_ip: Option<IpAddr>,
    state: &OAuthState,
) -> Result<TokenResponse, TokenError> {
    let refresh_token_str =
        extract_required_field(request.refresh_token.as_deref(), "refresh_token")?;
    let refresh_token = RefreshTokenId::new(refresh_token_str);

    let client_id = if let Some(id) = request.client_id.as_deref() {
        ClientId::new(id)
    } else {
        repo.find_client_id_from_refresh_token(&refresh_token)
            .await
            .map_err(|e| TokenError::ServerError {
                message: format!("Failed to lookup refresh token: {e}"),
            })?
            .ok_or_else(|| TokenError::InvalidRefreshToken {
                reason: "Invalid refresh token".to_owned(),
            })?
    };

    validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
        .await
        .map_err(|_e| TokenError::InvalidClientSecret)?;

    let consumed = repo
        .consume_refresh_token(&refresh_token, &client_id)
        .await
        .map_err(|e| TokenError::InvalidRefreshToken {
            reason: e.to_string(),
        })?;
    let user_id = consumed.user_id;
    let original_scope = consumed.scope;
    let family_id = consumed.family_id;

    let effective_scope = if let Some(requested_scope) = request.scope.as_deref() {
        let original_scopes = OAuthRepository::parse_scopes(&original_scope);
        let requested_scopes = OAuthRepository::parse_scopes(requested_scope);

        for requested in &requested_scopes {
            if !original_scopes.contains(requested) {
                return Err(TokenError::InvalidRequest {
                    field: "scope".to_owned(),
                    message: format!("Requested scope '{requested}' not in original scope"),
                });
            }
        }
        requested_scope
    } else {
        &original_scope
    };

    let generated = generate_tokens_by_user_id(
        &repo,
        TokenGenerationParams {
            client_id: &client_id,
            user_id: &user_id,
            scope: Some(effective_scope),
            headers,
            caller_ip,
            resource: request.resource.as_deref(),
            family_id: Some(family_id.as_str()),
        },
        state,
    )
    .await
    .map_err(|e| TokenError::ServerError {
        message: e.to_string(),
    })?;

    let token_response = generated.response;
    tracing::info!(
        grant_type = "refresh_token",
        client_id = %client_id,
        user_id = %user_id,
        scope = %effective_scope,
        token_type = %token_response.token_type,
        expires_in = token_response.expires_in,
        "Token issued"
    );

    Ok(token_response)
}

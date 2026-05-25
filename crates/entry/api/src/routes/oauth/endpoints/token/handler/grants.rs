//! Per-`grant_type` token issuance: authorization-code, refresh-token,
//! client-credentials, and RFC 8693 token-exchange.

use axum::http::HeaderMap;
use systemprompt_identifiers::{AuthorizationCode, ClientId, RefreshTokenId};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;

use super::super::generation::{
    ClientTokenOptions, TokenExchangeRequest, TokenGenerationParams, generate_client_tokens,
    generate_tokens_by_user_id, handle_token_exchange,
};
use super::super::validation::{
    AuthCodeValidationParams, extract_required_field, validate_authorization_code,
    validate_client_credentials,
};
use super::super::{TokenError, TokenRequest, TokenResponse};
use super::map_exchange_error;

pub(super) async fn handle_authorization_code_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    state: &OAuthState,
) -> Result<TokenResponse, TokenError> {
    let code_str = extract_required_field(request.code.as_deref(), "code")?;
    let code = AuthorizationCode::new(code_str);

    let client_id = if let Some(id) = request.client_id.as_deref() {
        ClientId::new(id)
    } else {
        repo.get_client_id_from_auth_code(&code)
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
    state: &OAuthState,
) -> Result<TokenResponse, TokenError> {
    let refresh_token_str =
        extract_required_field(request.refresh_token.as_deref(), "refresh_token")?;
    let refresh_token = RefreshTokenId::new(refresh_token_str);

    let client_id = if let Some(id) = request.client_id.as_deref() {
        ClientId::new(id)
    } else {
        repo.get_client_id_from_refresh_token(&refresh_token)
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

pub(super) async fn handle_token_exchange_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    state: &OAuthState,
) -> Result<TokenResponse, TokenError> {
    let subject_token = extract_required_field(request.subject_token.as_deref(), "subject_token")?;
    let subject_token_type =
        extract_required_field(request.subject_token_type.as_deref(), "subject_token_type")?;

    let client_id_str = extract_required_field(request.client_id.as_deref(), "client_id")?;
    let client_id = ClientId::new(client_id_str);
    validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
        .await
        .map_err(|_e| TokenError::InvalidClientSecret)?;

    let exchange = TokenExchangeRequest {
        subject_token,
        subject_token_type,
        actor_token: request.actor_token.as_deref(),
        actor_token_type: request.actor_token_type.as_deref(),
        requested_token_type: request.requested_token_type.as_deref(),
        scope: request.scope.as_deref(),
        audience: request.audience.as_deref(),
        resource: request.resource.as_deref(),
    };

    let response = handle_token_exchange(&repo, &client_id, exchange, headers, state)
        .await
        .map_err(|e| map_exchange_error(&e))?;

    tracing::info!(
        grant_type = "urn:ietf:params:oauth:grant-type:token-exchange",
        client_id = %client_id,
        scope = %response.scope.as_deref().unwrap_or(""),
        "Token exchanged"
    );

    Ok(response)
}

pub(super) async fn handle_client_credentials_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    state: &OAuthState,
) -> Result<TokenResponse, TokenError> {
    let client_id_str = extract_required_field(request.client_id.as_deref(), "client_id")?;
    let client_id = ClientId::new(client_id_str);

    validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
        .await
        .map_err(|_e| TokenError::InvalidClientSecret)?;

    let options = ClientTokenOptions {
        scope: request.scope.as_deref(),
        plugin_id: request.plugin_id.as_deref(),
        audience: request.audience.as_deref(),
    };
    let token_response = generate_client_tokens(&repo, &client_id, headers, state, options)
        .await
        .map_err(|e| TokenError::ServerError {
            message: e.to_string(),
        })?;

    tracing::info!(
        grant_type = "client_credentials",
        client_id = %client_id,
        scope = %token_response.scope.as_deref().unwrap_or(""),
        token_type = %token_response.token_type,
        expires_in = token_response.expires_in,
        "Token issued"
    );

    Ok(token_response)
}

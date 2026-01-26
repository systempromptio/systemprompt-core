use super::generation::{
    convert_token_result_to_response, generate_client_tokens, generate_tokens_by_user_id,
    TokenGenerationParams,
};
use super::validation::{
    extract_required_field, validate_authorization_code, validate_client_credentials,
};
use super::{TokenError, TokenRequest};
use axum::extract::{Extension, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::Form;
use std::sync::Arc;
use systemprompt_identifiers::{AuthorizationCode, ClientId, RefreshTokenId};
use systemprompt_models::RequestContext;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::{GrantType, OAuthState};
use tracing::instrument;

#[instrument(skip(state, _req_ctx, headers, request), fields(grant_type = %request.grant_type))]
pub async fn handle_token(
    Extension(_req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    headers: HeaderMap,
    Form(request): Form<TokenRequest>,
) -> impl IntoResponse {
    let repo = match OAuthRepository::new(Arc::clone(state.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };

    tracing::info!(grant_type = %request.grant_type, "Token request received");

    let response = match request.grant_type.parse::<GrantType>().map_err(|e| {
        tracing::debug!(grant_type = %request.grant_type, error = %e, "Failed to parse grant type");
        e
    }).ok() {
        Some(GrantType::AuthorizationCode) => {
            handle_authorization_code_grant(repo, request, &headers, &state).await
        },
        Some(GrantType::RefreshToken) => {
            handle_refresh_token_grant(repo, request, &headers, &state).await
        },
        Some(GrantType::ClientCredentials) => {
            handle_client_credentials_grant(repo, request, &state).await
        },
        None => {
            tracing::info!(
                client_id = ?request.client_id,
                grant_type = %request.grant_type,
                denial_reason = "unsupported_grant_type",
                error_code = "unsupported_grant_type",
                "Token request denied"
            );

            let error = TokenError::UnsupportedGrantType {
                grant_type: request.grant_type.clone(),
            };
            convert_token_result_to_response(Err(error))
        },
    };

    response
}

async fn handle_authorization_code_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    state: &OAuthState,
) -> axum::response::Response {
    let result = async {
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
                    reason: "Invalid or expired authorization code".to_string(),
                })?
        };

        validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
            .await
            .map_err(|_| TokenError::InvalidClientSecret)?;

        let (user_id, authorized_scope) = validate_authorization_code(
            &repo,
            &code,
            &client_id,
            request.redirect_uri.as_deref(),
            request.code_verifier.as_deref(),
        )
        .await
        .map_err(|e| TokenError::InvalidGrant {
            reason: e.to_string(),
        })?;

        let token_response = generate_tokens_by_user_id(
            &repo,
            TokenGenerationParams {
                client_id: &client_id,
                user_id: &user_id,
                scope: Some(&authorized_scope),
                headers,
            },
            state,
        )
        .await
        .map_err(|e| TokenError::ServerError {
            message: e.to_string(),
        })?;

        tracing::info!(
            grant_type = "authorization_code",
            client_id = %client_id,
            user_id = %user_id,
            scope = %authorized_scope,
            token_type = %token_response.token_type,
            expires_in = token_response.expires_in,
            "Token issued"
        );

        Ok(token_response)
    }
    .await;

    if let Err(ref error) = result {
        tracing::error!(
            error = %error,
            grant_type = "authorization_code",
            client_id = ?request.client_id,
            "Token request failed"
        );
    }

    convert_token_result_to_response(result)
}

async fn handle_refresh_token_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    state: &OAuthState,
) -> axum::response::Response {
    let result = async {
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
                    reason: "Invalid refresh token".to_string(),
                })?
        };

        validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
            .await
            .map_err(|_| TokenError::InvalidClientSecret)?;

        let (user_id, original_scope) = repo
            .consume_refresh_token(&refresh_token, &client_id)
            .await
            .map_err(|e| TokenError::InvalidRefreshToken {
                reason: e.to_string(),
            })?;

        let effective_scope = if let Some(requested_scope) = request.scope.as_deref() {
            let original_scopes = OAuthRepository::parse_scopes(&original_scope);
            let requested_scopes = OAuthRepository::parse_scopes(requested_scope);

            for requested in &requested_scopes {
                if !original_scopes.contains(requested) {
                    return Err(TokenError::InvalidRequest {
                        field: "scope".to_string(),
                        message: format!("Requested scope '{requested}' not in original scope"),
                    });
                }
            }
            requested_scope
        } else {
            &original_scope
        };

        let token_response = generate_tokens_by_user_id(
            &repo,
            TokenGenerationParams {
                client_id: &client_id,
                user_id: &user_id,
                scope: Some(effective_scope),
                headers,
            },
            state,
        )
        .await
        .map_err(|e| TokenError::ServerError {
            message: e.to_string(),
        })?;

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
    .await;

    if let Err(ref error) = result {
        tracing::error!(
            error = %error,
            grant_type = "refresh_token",
            client_id = ?request.client_id,
            "Token request failed"
        );
    }

    convert_token_result_to_response(result)
}

async fn handle_client_credentials_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    state: &OAuthState,
) -> axum::response::Response {
    let result = async {
        let client_id_str = extract_required_field(request.client_id.as_deref(), "client_id")?;
        let client_id = ClientId::new(client_id_str);

        validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
            .await
            .map_err(|_| TokenError::InvalidClientSecret)?;

        let token_response =
            generate_client_tokens(&repo, &client_id, request.scope.as_deref(), state)
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
    .await;

    if let Err(ref error) = result {
        tracing::error!(
            error = %error,
            grant_type = "client_credentials",
            client_id = ?request.client_id,
            "Token request failed"
        );
    }

    convert_token_result_to_response(result)
}

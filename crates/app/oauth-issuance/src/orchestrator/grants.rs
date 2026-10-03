//! User-bound grants: authorization-code redemption and refresh-token
//! rotation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AuthorizationCode, ClientId, RefreshTokenId};
use systemprompt_oauth::OauthError;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validation::validate_client_credentials;

use super::TokenIssuanceOrchestrator;
use crate::user_tokens::{UserTokenParams, generate_tokens_by_user_id};
use crate::validation::{
    AuthCodeValidationParams, extract_required_field, validate_authorization_code,
};
use crate::{IssuanceError, IssuanceResult, RequestOrigin, TokenRequest, TokenResponse};

impl TokenIssuanceOrchestrator<'_> {
    pub(super) async fn authorization_code(
        &self,
        request: TokenRequest,
        origin: RequestOrigin<'_>,
    ) -> IssuanceResult<TokenResponse> {
        let repo = self.repo();
        let code_str = extract_required_field(request.code.as_deref(), "code")?;
        let code = AuthorizationCode::new(code_str);

        let client_id = if let Some(id) = request.client_id.as_deref() {
            ClientId::new(id)
        } else {
            repo.find_client_id_from_auth_code(&code)
                .await?
                .ok_or_else(|| IssuanceError::InvalidGrant {
                    reason: "Invalid or expired authorization code".to_owned(),
                })?
        };

        let client =
            validate_client_credentials(repo, &client_id, request.client_secret.as_deref()).await?;
        require_public_client_redirect(&client.token_endpoint_auth_method, &request)?;

        let validation_result = validate_authorization_code(AuthCodeValidationParams {
            repo,
            code: &code,
            client_id: &client_id,
            redirect_uri: request.redirect_uri.as_deref(),
            code_verifier: request.code_verifier.as_deref(),
            request_resource: request.resource.as_deref(),
        })
        .await?;

        let generated = generate_tokens_by_user_id(
            repo,
            UserTokenParams {
                client_id: &client_id,
                user_id: &validation_result.user_id,
                scope: Some(&validation_result.scope),
                origin,
                resource: validation_result.resource.as_deref(),
                family_id: None,
            },
            self.state,
        )
        .await
        .map_err(|e| IssuanceError::server("Token generation failed", e))?;

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

    pub(super) async fn refresh_token(
        &self,
        request: TokenRequest,
        origin: RequestOrigin<'_>,
    ) -> IssuanceResult<TokenResponse> {
        let repo = self.repo();
        let refresh_token_str =
            extract_required_field(request.refresh_token.as_deref(), "refresh_token")?;
        let refresh_token = RefreshTokenId::new(refresh_token_str);

        let client_id = if let Some(id) = request.client_id.as_deref() {
            ClientId::new(id)
        } else {
            repo.find_client_id_from_refresh_token(&refresh_token)
                .await?
                .ok_or_else(|| IssuanceError::InvalidRefreshToken {
                    reason: "Invalid refresh token".to_owned(),
                })?
        };

        validate_client_credentials(repo, &client_id, request.client_secret.as_deref()).await?;

        let consumed = repo
            .consume_refresh_token(&refresh_token, &client_id)
            .await
            .map_err(refresh_token_error)?;
        let user_id = consumed.user_id;
        let original_scope = consumed.scope;
        let family_id = consumed.family_id;

        let effective_scope = narrow_scope(request.scope.as_deref(), &original_scope)?;

        let generated = generate_tokens_by_user_id(
            repo,
            UserTokenParams {
                client_id: &client_id,
                user_id: &user_id,
                scope: Some(effective_scope),
                origin,
                resource: request.resource.as_deref(),
                family_id: Some(family_id.as_str()),
            },
            self.state,
        )
        .await
        .map_err(|e| IssuanceError::server("Token generation failed", e))?;

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
}

// Why: RFC 6749 §4.1.3 — a public client has no secret binding the code to it,
// so the redirect_uri echo is the only proof it is the same party that started
// the flow; it may not be omitted to skip the comparison.
fn require_public_client_redirect(auth_method: &str, request: &TokenRequest) -> IssuanceResult<()> {
    if auth_method == "none" && request.redirect_uri.is_none() {
        return Err(IssuanceError::InvalidRequest {
            field: "redirect_uri".to_owned(),
            message: "required for public clients".to_owned(),
        });
    }
    Ok(())
}

fn narrow_scope<'a>(requested: Option<&'a str>, original: &'a str) -> IssuanceResult<&'a str> {
    let Some(requested_scope) = requested else {
        return Ok(original);
    };
    let original_scopes = OAuthRepository::parse_scopes(original);
    for requested in &OAuthRepository::parse_scopes(requested_scope) {
        if !original_scopes.contains(requested) {
            return Err(IssuanceError::InvalidRequest {
                field: "scope".to_owned(),
                message: format!("Requested scope '{requested}' not in original scope"),
            });
        }
    }
    Ok(requested_scope)
}

fn refresh_token_error(error: OauthError) -> IssuanceError {
    match error {
        OauthError::TokenInvalid(reason) | OauthError::Expired(reason) => {
            IssuanceError::InvalidRefreshToken { reason }
        },
        other => IssuanceError::Oauth(other),
    }
}

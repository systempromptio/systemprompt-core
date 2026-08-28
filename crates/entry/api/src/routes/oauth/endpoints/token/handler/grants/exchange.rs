//! Grants that mint tokens without an authorization code: RFC 8693
//! token-exchange, the RFC 7523 jwt-bearer ID-JAG redemption, and
//! client-credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use std::net::IpAddr;
use systemprompt_identifiers::ClientId;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validation::id_jag::ID_JAG_TOKEN_TYPE;

use super::super::super::generation::{
    ClientCredentialsError, ClientTokenOptions, RequestOrigin, TokenExchangeRequest,
    generate_client_tokens, handle_token_exchange,
};
use super::super::super::validation::{extract_required_field, validate_client_credentials};
use super::super::super::{TokenError, TokenRequest, TokenResponse};
use super::super::map_exchange_error;

pub(in crate::routes::oauth::endpoints::token::handler) async fn handle_token_exchange_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    caller_ip: Option<IpAddr>,
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

    let origin = RequestOrigin { headers, caller_ip };
    let response = handle_token_exchange(&repo, &client_id, exchange, origin, state)
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

// Why: RFC 7523 assertion grant, the redemption leg of Enterprise-Managed
// Authorization: the client presents the ID-JAG its `IdP` issued and
// receives an access token for the employee it names. Shares its validator
// with the equivalent token-exchange call, which stays available.
pub(in crate::routes::oauth::endpoints::token::handler) async fn handle_jwt_bearer_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    caller_ip: Option<IpAddr>,
    state: &OAuthState,
) -> Result<TokenResponse, TokenError> {
    let assertion = extract_required_field(request.assertion.as_deref(), "assertion")?;

    let client_id_str = extract_required_field(request.client_id.as_deref(), "client_id")?;
    let client_id = ClientId::new(client_id_str);
    validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
        .await
        .map_err(|_e| TokenError::InvalidClientSecret)?;

    let exchange = TokenExchangeRequest {
        subject_token: assertion,
        subject_token_type: ID_JAG_TOKEN_TYPE,
        scope: request.scope.as_deref(),
        audience: request.audience.as_deref(),
        resource: request.resource.as_deref(),
        ..Default::default()
    };

    let origin = RequestOrigin { headers, caller_ip };
    let response = handle_token_exchange(&repo, &client_id, exchange, origin, state)
        .await
        .map_err(|e| map_exchange_error(&e))?;

    tracing::info!(
        grant_type = "urn:ietf:params:oauth:grant-type:jwt-bearer",
        client_id = %client_id,
        scope = %response.scope.as_deref().unwrap_or(""),
        "ID-JAG redeemed"
    );

    Ok(response)
}

pub(in crate::routes::oauth::endpoints::token::handler) async fn handle_client_credentials_grant(
    repo: OAuthRepository,
    request: TokenRequest,
    headers: &HeaderMap,
    caller_ip: Option<IpAddr>,
    state: &OAuthState,
) -> Result<TokenResponse, TokenError> {
    let client_id_str = extract_required_field(request.client_id.as_deref(), "client_id")?;
    let client_id = ClientId::new(client_id_str);

    validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
        .await
        .map_err(|e| {
            tracing::warn!(
                client_id = %client_id,
                error = %e,
                "client_credentials grant rejected"
            );
            TokenError::InvalidClientSecret
        })?;

    let options = ClientTokenOptions {
        scope: request.scope.as_deref(),
        plugin_id: request.plugin_id.as_deref(),
        audience: request.audience.as_deref(),
    };
    let origin = RequestOrigin { headers, caller_ip };
    let token_response = generate_client_tokens(&repo, &client_id, origin, state, options)
        .await
        .map_err(|e| map_client_credentials_error(&client_id, e))?;

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

fn map_client_credentials_error(client_id: &ClientId, error: ClientCredentialsError) -> TokenError {
    tracing::warn!(
        client_id = %client_id,
        error = %error,
        "client_credentials token generation failed"
    );
    match error {
        ClientCredentialsError::ClientNotFound
        | ClientCredentialsError::OwnerNotFound
        | ClientCredentialsError::OwnerInactive => TokenError::InvalidClient,
        ClientCredentialsError::InvalidScope(message) => TokenError::InvalidScope { message },
        ClientCredentialsError::HookScopeRequiresHookAudience => TokenError::InvalidScope {
            message: "hook scopes require audience=hook on the token request".to_owned(),
        },
        ClientCredentialsError::InvalidAudience(message) => TokenError::InvalidTarget { message },
        err @ (ClientCredentialsError::OwnerIdMalformed(_)
        | ClientCredentialsError::UserProviderUnavailable(_)
        | ClientCredentialsError::SessionCreate(_)
        | ClientCredentialsError::JwtSign(_)
        | ClientCredentialsError::ConfigUnavailable(_)) => TokenError::ServerError {
            message: err.to_string(),
        },
    }
}

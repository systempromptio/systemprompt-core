//! Grants that mint tokens without an authorization code: RFC 8693
//! token-exchange, the RFC 7523 jwt-bearer ID-JAG redemption, and
//! client-credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ClientId, PluginId};
use systemprompt_oauth::services::validation::id_jag::ID_JAG_TOKEN_TYPE;
use systemprompt_oauth::services::validation::validate_client_credentials;

use super::TokenIssuanceOrchestrator;
use crate::client_credentials::{
    ClientCredentialsError, ClientTokenOptions, generate_client_tokens,
};
use crate::token_exchange::{TokenExchangeRequest, handle_token_exchange};
use crate::validation::extract_required_field;
use crate::{IssuanceError, IssuanceResult, RequestOrigin, TokenRequest, TokenResponse};

impl TokenIssuanceOrchestrator<'_> {
    async fn authenticated_client(&self, request: &TokenRequest) -> IssuanceResult<ClientId> {
        let client_id_str = extract_required_field(request.client_id.as_deref(), "client_id")?;
        let client_id = ClientId::new(client_id_str);
        validate_client_credentials(self.repo(), &client_id, request.client_secret.as_deref())
            .await?;
        Ok(client_id)
    }

    pub(super) async fn token_exchange(
        &self,
        request: TokenRequest,
        origin: RequestOrigin<'_>,
    ) -> IssuanceResult<TokenResponse> {
        let subject_token =
            extract_required_field(request.subject_token.as_deref(), "subject_token")?;
        let subject_token_type =
            extract_required_field(request.subject_token_type.as_deref(), "subject_token_type")?;
        let client_id = self.authenticated_client(&request).await?;

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

        let response =
            handle_token_exchange(self.repo(), &client_id, exchange, origin, self.state).await?;

        tracing::info!(
            grant_type = "urn:ietf:params:oauth:grant-type:token-exchange",
            client_id = %client_id,
            scope = %response.scope.as_deref().unwrap_or(""),
            "Token exchanged"
        );

        Ok(response)
    }

    pub(super) async fn jwt_bearer(
        &self,
        request: TokenRequest,
        origin: RequestOrigin<'_>,
    ) -> IssuanceResult<TokenResponse> {
        let assertion = extract_required_field(request.assertion.as_deref(), "assertion")?;
        let client_id = self.authenticated_client(&request).await?;

        let exchange = TokenExchangeRequest {
            subject_token: assertion,
            subject_token_type: ID_JAG_TOKEN_TYPE,
            scope: request.scope.as_deref(),
            audience: request.audience.as_deref(),
            resource: request.resource.as_deref(),
            ..Default::default()
        };

        let response =
            handle_token_exchange(self.repo(), &client_id, exchange, origin, self.state).await?;

        tracing::info!(
            grant_type = "urn:ietf:params:oauth:grant-type:jwt-bearer",
            client_id = %client_id,
            scope = %response.scope.as_deref().unwrap_or(""),
            "ID-JAG redeemed"
        );

        Ok(response)
    }

    pub(super) async fn client_credentials(
        &self,
        request: TokenRequest,
        origin: RequestOrigin<'_>,
    ) -> IssuanceResult<TokenResponse> {
        let client_id = self.authenticated_client(&request).await?;

        let options = ClientTokenOptions {
            scope: request.scope.as_deref(),
            plugin_id: request.plugin_id.as_deref().map(PluginId::new),
            audience: request.audience.as_deref(),
        };
        let token_response =
            generate_client_tokens(self.repo(), &client_id, origin, self.state, options)
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
}

fn map_client_credentials_error(
    client_id: &ClientId,
    error: ClientCredentialsError,
) -> IssuanceError {
    tracing::warn!(
        client_id = %client_id,
        error = %error,
        "client_credentials token generation failed"
    );
    match error {
        ClientCredentialsError::ClientNotFound
        | ClientCredentialsError::OwnerNotFound
        | ClientCredentialsError::OwnerInactive => IssuanceError::InvalidClient,
        ClientCredentialsError::InvalidScope(message) => IssuanceError::InvalidScope { message },
        ClientCredentialsError::UnparseableScope(_) => IssuanceError::InvalidScope {
            message: "scope contains an unknown permission".to_owned(),
        },
        ClientCredentialsError::HookScopeRequiresHookAudience => IssuanceError::InvalidScope {
            message: "hook scopes require audience=hook on the token request".to_owned(),
        },
        ClientCredentialsError::InvalidAudience(message) => {
            IssuanceError::InvalidTarget { message }
        },
        ClientCredentialsError::UnknownAudience { audience, .. } => IssuanceError::InvalidTarget {
            message: format!("'{audience}' is not a known audience"),
        },
        err @ (ClientCredentialsError::UserProviderUnavailable(_)
        | ClientCredentialsError::SessionCreate(_)
        | ClientCredentialsError::JwtSign(_)
        | ClientCredentialsError::ConfigUnavailable(_)) => {
            IssuanceError::server("Client credentials token generation failed", err)
        },
    }
}

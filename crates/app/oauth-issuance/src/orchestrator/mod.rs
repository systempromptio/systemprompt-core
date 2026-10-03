//! Grant-type dispatch for the token endpoint.
//!
//! [`TokenIssuanceOrchestrator`] parses `grant_type` and runs the matching
//! flow: authorization-code redemption and refresh-token rotation (`grants`),
//! and the client-credentials, RFC 8693 token-exchange and RFC 7523
//! jwt-bearer flows (`exchange`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod exchange;
mod grants;

use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::{GrantType, OAuthState};

use crate::{IssuanceError, IssuanceResult, RequestOrigin, TokenRequest, TokenResponse};

/// Issues tokens for one token-endpoint request against the OAuth state.
#[derive(Debug, Clone, Copy)]
pub struct TokenIssuanceOrchestrator<'a> {
    state: &'a OAuthState,
}

impl<'a> TokenIssuanceOrchestrator<'a> {
    pub const fn new(state: &'a OAuthState) -> Self {
        Self { state }
    }

    pub async fn issue(
        &self,
        request: TokenRequest,
        origin: RequestOrigin<'_>,
    ) -> IssuanceResult<TokenResponse> {
        let grant_type = request
            .grant_type
            .parse::<GrantType>()
            .map_err(|_unknown| IssuanceError::UnsupportedGrantType {
                grant_type: request.grant_type.clone(),
            })?;
        match grant_type {
            GrantType::AuthorizationCode => self.authorization_code(request, origin).await,
            GrantType::RefreshToken => self.refresh_token(request, origin).await,
            GrantType::ClientCredentials => self.client_credentials(request, origin).await,
            GrantType::TokenExchange => self.token_exchange(request, origin).await,
            GrantType::JwtBearer => self.jwt_bearer(request, origin).await,
        }
    }

    const fn repo(&self) -> &OAuthRepository {
        self.state.oauth_repository()
    }
}

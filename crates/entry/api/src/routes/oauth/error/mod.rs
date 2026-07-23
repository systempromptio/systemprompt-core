//! Unified HTTP error type for the OAuth route module.
//!
//! Every OAuth handler returns `Result<_, OAuthHttpError>`. The `IntoResponse`
//! impl logs exactly once (matching `ApiError`'s log-by-status-class pattern)
//! and emits an RFC 6749 §5.2 wire shape `{"error": "...", "error_description":
//! "..."}`. The authorize-flow variant (§4.1.2.1) carries a redirect target so
//! the response renders as a 302 to the client's `redirect_uri` with the same
//! error fields encoded as query parameters.
//!
//! `From` impls (in the `conversions` submodule) bridge the underlying domain
//! errors (`OauthError`, `AuthProviderError`, `SecretsBootstrapError`) so
//! handlers use `?` and the variant-to-RFC-code mapping lives in one place.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Serialize;

mod code;
mod conversions;

pub use code::OAuthErrorCode;

#[derive(Debug, Clone)]
pub struct RedirectContext {
    pub uri: String,
    pub state: Option<String>,
}

#[derive(Debug)]
pub struct OAuthHttpError {
    code: OAuthErrorCode,
    status: StatusCode,
    description: String,
    redirect: Option<RedirectContext>,
}

impl OAuthHttpError {
    #[must_use]
    pub fn new(code: OAuthErrorCode, description: impl Into<String>) -> Self {
        Self {
            status: code.default_status(),
            code,
            description: description.into(),
            redirect: None,
        }
    }

    #[must_use]
    pub fn invalid_request(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::InvalidRequest, description)
    }

    #[must_use]
    pub fn invalid_client(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::InvalidClient, description)
    }

    #[must_use]
    pub fn invalid_grant(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::InvalidGrant, description)
    }

    #[must_use]
    pub fn unauthorized_client(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::UnauthorizedClient, description)
    }

    #[must_use]
    pub fn unsupported_grant_type(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::UnsupportedGrantType, description)
    }

    #[must_use]
    pub fn invalid_scope(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::InvalidScope, description)
    }

    #[must_use]
    pub fn invalid_token(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::InvalidToken, description)
    }

    #[must_use]
    pub fn access_denied(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::AccessDenied, description)
    }

    #[must_use]
    pub fn server_error(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::ServerError, description)
    }

    #[must_use]
    pub fn invalid_client_metadata(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::InvalidClientMetadata, description)
    }

    #[must_use]
    pub fn authentication_failed(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::AuthenticationFailed, description)
    }

    #[must_use]
    pub fn registration_failed(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::RegistrationFailed, description)
    }

    #[must_use]
    pub fn username_unavailable(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::UsernameUnavailable, description)
    }

    #[must_use]
    pub fn email_exists(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::EmailExists, description)
    }

    #[must_use]
    pub fn expired_challenge(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::ExpiredChallenge, description)
    }

    #[must_use]
    pub fn invalid_credential(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::InvalidCredential, description)
    }

    #[must_use]
    pub fn link_failed(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::LinkFailed, description)
    }

    #[must_use]
    pub fn invalid_target(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::InvalidTarget, description)
    }

    #[must_use]
    pub fn not_found(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::NotFound, description)
    }

    #[must_use]
    pub const fn with_status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    #[must_use]
    pub fn with_redirect(mut self, uri: impl Into<String>, state: Option<String>) -> Self {
        self.redirect = Some(RedirectContext {
            uri: uri.into(),
            state,
        });
        self
    }

    #[must_use]
    pub const fn code(&self) -> OAuthErrorCode {
        self.code
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    fn log(&self) {
        if self.status.is_server_error() {
            tracing::error!(
                error = self.code.as_str(),
                description = %self.description,
                status = self.status.as_u16(),
                "OAuth server error response"
            );
        } else if self.status.is_client_error() {
            tracing::warn!(
                error = self.code.as_str(),
                description = %self.description,
                status = self.status.as_u16(),
                "OAuth client error response"
            );
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthErrorBody<'a> {
    error: &'a str,
    error_description: &'a str,
}

impl IntoResponse for OAuthHttpError {
    fn into_response(self) -> Response {
        self.log();

        if let Some(redirect) = &self.redirect {
            let mut target = format!(
                "{}?error={}&error_description={}",
                redirect.uri,
                urlencoding::encode(self.code.as_str()),
                urlencoding::encode(&self.description),
            );
            if let Some(state) = &redirect.state {
                target.push_str("&state=");
                target.push_str(&urlencoding::encode(state));
            }
            return Redirect::to(&target).into_response();
        }

        let body = OAuthErrorBody {
            error: self.code.as_str(),
            error_description: &self.description,
        };
        let mut response = (self.status, Json(body)).into_response();

        if self.status == StatusCode::UNAUTHORIZED
            && let Ok(value) = HeaderValue::from_str(
                "Bearer resource_metadata=\"/.well-known/oauth-protected-resource\"",
            )
        {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, value);
        }

        response
    }
}

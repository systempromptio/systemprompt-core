//! Unified HTTP error type for the OAuth route module.
//!
//! Every OAuth handler returns `Result<_, OAuthHttpError>`. The `IntoResponse`
//! impl logs exactly once (matching `ApiError`'s log-by-status-class pattern)
//! and emits an RFC 6749 §5.2 wire shape `{"error": "...", "error_description":
//! "..."}`. The authorize-flow variant (§4.1.2.1) carries a redirect target so
//! the response renders as a 302 to the client's `redirect_uri` with the same
//! error fields encoded as query parameters.
//!
//! `From` impls bridge the underlying domain errors (`OauthError`,
//! `AuthProviderError`, `SecretsBootstrapError`) so handlers use `?` and the
//! variant-to-RFC-code mapping lives in one place.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Serialize;
use systemprompt_config::SecretsBootstrapError;
use systemprompt_oauth::OauthError;
use systemprompt_traits::auth::AuthProviderError;

/// RFC 6749 §5.2 error codes plus the WebAuthn/RFC 7591 extensions used by
/// this server. `Display` returns the wire string, so the enum doubles as the
/// `error` field source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthErrorCode {
    InvalidRequest,
    InvalidClient,
    InvalidGrant,
    UnauthorizedClient,
    UnsupportedGrantType,
    InvalidScope,
    AccessDenied,
    ServerError,
    TemporarilyUnavailable,
    InvalidClientMetadata,
    AuthenticationFailed,
    RegistrationFailed,
    UsernameUnavailable,
    EmailExists,
    ExpiredChallenge,
    InvalidCredential,
    NotFound,
}

impl OAuthErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidClient => "invalid_client",
            Self::InvalidGrant => "invalid_grant",
            Self::UnauthorizedClient => "unauthorized_client",
            Self::UnsupportedGrantType => "unsupported_grant_type",
            Self::InvalidScope => "invalid_scope",
            Self::AccessDenied => "access_denied",
            Self::ServerError => "server_error",
            Self::TemporarilyUnavailable => "temporarily_unavailable",
            Self::InvalidClientMetadata => "invalid_client_metadata",
            Self::AuthenticationFailed => "authentication_failed",
            Self::RegistrationFailed => "registration_failed",
            Self::UsernameUnavailable => "username_unavailable",
            Self::EmailExists => "email_exists",
            Self::ExpiredChallenge => "expired_challenge",
            Self::InvalidCredential => "invalid_credential",
            Self::NotFound => "not_found",
        }
    }

    #[must_use]
    pub const fn default_status(self) -> StatusCode {
        match self {
            Self::InvalidRequest
            | Self::UnsupportedGrantType
            | Self::InvalidScope
            | Self::InvalidClientMetadata
            | Self::ExpiredChallenge
            | Self::InvalidCredential => StatusCode::BAD_REQUEST,
            Self::InvalidClient
            | Self::InvalidGrant
            | Self::UnauthorizedClient
            | Self::AccessDenied
            | Self::AuthenticationFailed => StatusCode::UNAUTHORIZED,
            Self::RegistrationFailed => StatusCode::BAD_REQUEST,
            Self::UsernameUnavailable | Self::EmailExists => StatusCode::CONFLICT,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::TemporarilyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

/// Authorize-flow redirect context (RFC 6749 §4.1.2.1).
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
    pub fn not_found(description: impl Into<String>) -> Self {
        Self::new(OAuthErrorCode::NotFound, description)
    }

    /// Override the default HTTP status. Use sparingly — the per-code default
    /// already encodes the spec mapping.
    #[must_use]
    pub const fn with_status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Attach RFC 6749 §4.1.2.1 redirect context. When set, `IntoResponse`
    /// emits a 302 to `uri?error=...&error_description=...&state=...` instead
    /// of a JSON body.
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
            && let Ok(value) =
                HeaderValue::from_str("Bearer resource_metadata=\"/.well-known/oauth-protected-resource\"")
        {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, value);
        }

        response
    }
}

impl From<OauthError> for OAuthHttpError {
    fn from(err: OauthError) -> Self {
        match &err {
            OauthError::InvalidClient(_) | OauthError::ClientNotFound(_) => {
                Self::invalid_client(err.to_string())
            },
            OauthError::InvalidGrant(_)
            | OauthError::CodeNotFound(_)
            | OauthError::TokenNotFound(_)
            | OauthError::PkceMismatch(_)
            | OauthError::Expired(_) => Self::invalid_grant(err.to_string()),
            OauthError::Validation(_) => Self::invalid_request(err.to_string()),
            OauthError::Unauthorized(_) => Self::access_denied(err.to_string()),
            OauthError::WebAuthn(_)
            | OauthError::User(_)
            | OauthError::Session(_)
            | OauthError::Token(_)
            | OauthError::Provider(_)
            | OauthError::Repository(_)
            | OauthError::DatabaseRepository(_)
            | OauthError::Config(_)
            | OauthError::Crypto(_)
            | OauthError::Internal(_) => Self::server_error(err.to_string()),
        }
    }
}

impl From<AuthProviderError> for OAuthHttpError {
    fn from(err: AuthProviderError) -> Self {
        match &err {
            AuthProviderError::InvalidCredentials | AuthProviderError::InvalidToken => {
                Self::invalid_client(err.to_string())
            },
            AuthProviderError::UserNotFound => Self::not_found(err.to_string()),
            AuthProviderError::TokenExpired => Self::invalid_grant(err.to_string()),
            AuthProviderError::InsufficientPermissions => Self::access_denied(err.to_string()),
            AuthProviderError::Internal(_) => Self::server_error(err.to_string()),
            _ => Self::server_error(err.to_string()),
        }
    }
}

impl From<SecretsBootstrapError> for OAuthHttpError {
    fn from(err: SecretsBootstrapError) -> Self {
        Self::server_error(err.to_string())
    }
}

impl From<sqlx::Error> for OAuthHttpError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db_err) = &err
            && db_err.is_unique_violation()
        {
            return Self::new(OAuthErrorCode::UsernameUnavailable, err.to_string());
        }
        Self::server_error(err.to_string())
    }
}

impl From<anyhow::Error> for OAuthHttpError {
    fn from(err: anyhow::Error) -> Self {
        Self::server_error(err.to_string())
    }
}

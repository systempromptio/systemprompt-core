//! What a forwarded request can fail with, and the status each failure maps
//! to for the loopback caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use hyper::StatusCode;
use thiserror::Error;

use super::BUFFERED_BODY_LIMIT;
use super::replay::describe;

#[derive(Debug, Error)]
pub enum ForwardError {
    #[error("routing unavailable: {0}")]
    Routing(String),
    #[error("authentication unavailable: {0}")]
    Auth(String),
    #[error("authentication temporarily unavailable, retrying: {0}")]
    AuthRetryable(String),
    #[error("authentication timed out after 10s")]
    AuthTimeout,
    #[error("authentication unavailable: {0}")]
    Chain(#[source] Box<crate::auth::ChainError>),
    #[error("authentication unavailable: {0}")]
    Config(#[source] crate::config::ConfigReadError),
    #[error("authentication unavailable: {0}")]
    CredentialIdentity(#[source] std::io::Error),
    #[error("authentication unavailable: credential stamp task: {0}")]
    CredentialTask(#[source] tokio::task::JoinError),
    #[error("authentication unavailable: hook token mint for {plugin_id}: {source}")]
    HookToken {
        plugin_id: systemprompt_identifiers::PluginId,
        #[source]
        source: crate::auth::plugin_oauth::PluginOAuthError,
    },
    #[error("credential out of scope: a {presented} credential cannot drive {route}")]
    Scope {
        presented: &'static str,
        route: &'static str,
    },
    #[error("invalid request method {method}: {source}")]
    BadMethod {
        method: String,
        #[source]
        source: http::method::InvalidMethod,
    },
    #[error("invalid header value: {0}")]
    BadHeader(String),
    #[error("invalid header value: {name}: {source}")]
    InvalidHeader {
        name: String,
        #[source]
        source: HeaderBuildError,
    },
    #[error("upstream request failed: {}", describe(.0))]
    Upstream(#[from] reqwest::Error),
    #[error("response build failed: {0}")]
    BuildResponse(#[from] http::Error),
    #[error("request body exceeds {BUFFERED_BODY_LIMIT} bytes")]
    BodyTooLarge,
    #[error("request body read failed: {0}")]
    ReadBody(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl ForwardError {
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::Auth(_)
            | Self::AuthRetryable(_)
            | Self::AuthTimeout
            | Self::Chain(_)
            | Self::Config(_)
            | Self::CredentialIdentity(_)
            | Self::CredentialTask(_)
            | Self::HookToken { .. }
            | Self::Routing(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Scope { .. } => StatusCode::UNAUTHORIZED,
            Self::BodyTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::BadMethod { .. } | Self::BadHeader(_) | Self::InvalidHeader { .. } => {
                StatusCode::BAD_REQUEST
            },
            Self::Upstream(_) | Self::BuildResponse(_) | Self::ReadBody(_) => {
                StatusCode::BAD_GATEWAY
            },
        }
    }

    #[must_use]
    pub fn is_credential_failure(&self) -> bool {
        match self {
            Self::Auth(_)
            | Self::AuthTimeout
            | Self::Config(_)
            | Self::CredentialIdentity(_)
            | Self::CredentialTask(_)
            | Self::HookToken { .. } => true,
            Self::Chain(e) => e.is_terminal(),
            _ => false,
        }
    }

    #[must_use]
    pub fn is_retryable_auth(&self) -> bool {
        match self {
            Self::AuthRetryable(_) => true,
            Self::Chain(e) => !e.is_terminal(),
            _ => false,
        }
    }

    pub fn client_detail(&self) -> String {
        format!("{self}\n")
    }
}

#[derive(Debug, Error)]
pub enum HeaderBuildError {
    #[error(transparent)]
    Name(#[from] http::header::InvalidHeaderName),
    #[error(transparent)]
    Value(#[from] http::header::InvalidHeaderValue),
}

pub type ForwardResult<T> = Result<T, ForwardError>;

// Why: reqwest reports a socket the peer closed mid-exchange as a request
// error whose hyper cause is `IncompleteMessage`; that is the caller hanging
// up, not an upstream fault, and is logged at warn rather than error.
#[must_use]
pub fn is_client_disconnect(err: &ForwardError) -> bool {
    let ForwardError::Upstream(e) = err else {
        return false;
    };
    if !e.is_request() {
        return false;
    }
    let mut cause = std::error::Error::source(e);
    while let Some(inner) = cause {
        if let Some(h) = inner.downcast_ref::<hyper::Error>() {
            return h.is_incomplete_message() || h.is_closed();
        }
        cause = inner.source();
    }
    false
}

//! The rejection a gateway message route answers with before or instead of a
//! provider response.
//!
//! A [`RejectionError`] carries the status, the public message the client sees
//! for a 4xx, whether the rejection still owes an audit row, and — for a
//! failure the gateway caused — the underlying error, which is logged and never
//! rendered. A stable `error_key` (for example `context_window_exceeded`) is
//! added to the rendered `error` object when set. Every 5xx renders [`GATEWAY_SERVER_ERROR_MESSAGE`] whatever it
//! was built with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use systemprompt_traits::BoxedSource;

pub const GATEWAY_SERVER_ERROR_MESSAGE: &str = "The gateway could not complete the request";

#[derive(Debug)]
pub struct RejectionError {
    pub status: StatusCode,
    pub message: String,
    pub persist: bool,
    pub cause: Option<BoxedSource>,
    pub error_key: Option<&'static str>,
}

impl RejectionError {
    pub fn client(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            persist: true,
            cause: None,
            error_key: None,
        }
    }

    pub fn invalid<E>(status: StatusCode, error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            status,
            message: error.to_string(),
            persist: true,
            cause: Some(Box::new(error)),
            error_key: None,
        }
    }

    pub fn server(status: StatusCode, context: &'static str) -> Self {
        Self::client(status, context)
    }

    #[must_use]
    pub fn with_cause(mut self, cause: impl Into<BoxedSource>) -> Self {
        self.cause = Some(cause.into());
        self
    }

    #[must_use]
    pub const fn with_error_key(mut self, key: &'static str) -> Self {
        self.error_key = Some(key);
        self
    }

    #[must_use]
    pub const fn with_persist(mut self, persist: bool) -> Self {
        self.persist = persist;
        self
    }

    #[must_use]
    pub fn public_message(&self) -> &str {
        if self.status.is_server_error() {
            GATEWAY_SERVER_ERROR_MESSAGE
        } else {
            &self.message
        }
    }
}

impl axum::response::IntoResponse for RejectionError {
    fn into_response(self) -> axum::response::Response {
        if self.status.is_server_error() {
            tracing::error!(
                status = %self.status,
                message = %self.message,
                cause = ?self.cause,
                "Gateway request rejected"
            );
        } else {
            tracing::warn!(
                status = %self.status,
                message = %self.message,
                cause = ?self.cause,
                "Gateway request rejected"
            );
        }
        super::dispatch::errors::build_error_response(
            self.status,
            systemprompt_gateway::protocol::inbound::error_type_for_status(self.status),
            self.public_message(),
        )
    }
}

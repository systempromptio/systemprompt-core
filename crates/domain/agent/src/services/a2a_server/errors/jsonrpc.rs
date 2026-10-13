//! JSON-RPC 2.0 error envelope construction.
//!
//! [`JsonRpcErrorBuilder`] assembles spec-coded error responses. Every error
//! carries one [`A2aErrorInfo`], serialised as the single entry of the
//! `error.data` array A2A 1.0 requires; [`unauthorized_response`] and
//! [`forbidden_response`] are the auth-failure shortcuts. A request whose id
//! cannot be read is answered with an [`UnidentifiedErrorResponse`] (`id:
//! null`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::a2a::jsonrpc::{
    JSON_RPC_VERSION_2_0, JsonRpcError, JsonRpcResponse, NumberOrString,
};
use axum::http::StatusCode;
use serde::Serialize;
use systemprompt_identifiers::TaskId;
use systemprompt_logging::LogLevel;
use systemprompt_models::a2a::PROTOCOL_VERSION;

pub use super::codes::{
    CONTENT_TYPE_NOT_SUPPORTED, EXTENDED_AGENT_CARD_NOT_CONFIGURED, EXTENSION_SUPPORT_REQUIRED,
    INTERNAL_ERROR, INVALID_AGENT_RESPONSE, INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND,
    PARSE_ERROR, PUSH_NOTIFICATION_NOT_SUPPORTED, TASK_NOT_CANCELABLE, TASK_NOT_FOUND,
    UNSUPPORTED_OPERATION, VERSION_NOT_SUPPORTED, http_status_for,
};
use super::error_info::{A2aErrorInfo, A2aErrorReason};

pub type JsonRpcErrorResponse = JsonRpcResponse<()>;

#[derive(Debug, Clone, Serialize)]
pub struct UnidentifiedErrorResponse {
    pub jsonrpc: String,
    pub error: JsonRpcError,
    pub id: Option<NumberOrString>,
}


#[derive(Debug)]
pub struct JsonRpcErrorBuilder {
    code: i32,
    message: String,
    error_info: A2aErrorInfo,
    log_message: Option<String>,
    log_level: LogLevel,
}

impl JsonRpcErrorBuilder {
    pub fn new(code: i32, message: impl Into<String>, reason: A2aErrorReason) -> Self {
        Self {
            code,
            message: message.into(),
            error_info: A2aErrorInfo::new(reason),
            log_message: None,
            log_level: LogLevel::Error,
        }
    }

    pub fn with_error_info(mut self, error_info: A2aErrorInfo) -> Self {
        self.error_info = error_info;
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.error_info = self.error_info.with_metadata(key, value);
        self
    }

    pub const fn code(&self) -> i32 {
        self.code
    }

    pub const fn error_info(&self) -> &A2aErrorInfo {
        &self.error_info
    }

    pub fn with_log(mut self, message: impl Into<String>, level: LogLevel) -> Self {
        self.log_message = Some(message.into());
        self.log_level = level;
        self
    }

    pub fn log_error(mut self, message: impl Into<String>) -> Self {
        self.log_message = Some(message.into());
        self.log_level = LogLevel::Error;
        self
    }

    pub fn log_warn(mut self, message: impl Into<String>) -> Self {
        self.log_message = Some(message.into());
        self.log_level = LogLevel::Warn;
        self
    }

    pub fn build(self, request_id: &NumberOrString) -> JsonRpcErrorResponse {
        self.build_as(request_id)
    }

    pub fn build_as<T>(self, request_id: &NumberOrString) -> JsonRpcResponse<T> {
        JsonRpcResponse {
            jsonrpc: JSON_RPC_VERSION_2_0.to_owned(),
            result: None,
            error: Some(self.into_error()),
            id: request_id.clone(),
        }
    }

    pub fn build_unidentified(self) -> UnidentifiedErrorResponse {
        UnidentifiedErrorResponse {
            jsonrpc: JSON_RPC_VERSION_2_0.to_owned(),
            error: self.into_error(),
            id: None,
        }
    }

    pub fn into_error(self) -> JsonRpcError {
        if let Some(log_msg) = self.log_message {
            match self.log_level {
                LogLevel::Error => {
                    tracing::error!(topic = "a2a_jsonrpc", detail = %log_msg, "JSON-RPC error response");
                },
                LogLevel::Warn => {
                    tracing::warn!(topic = "a2a_jsonrpc", detail = %log_msg, "JSON-RPC error response");
                },
                LogLevel::Info => {
                    tracing::info!(topic = "a2a_jsonrpc", detail = %log_msg, "JSON-RPC error response");
                },
                LogLevel::Debug => {
                    tracing::debug!(topic = "a2a_jsonrpc", detail = %log_msg, "JSON-RPC error response");
                },
                LogLevel::Trace => {
                    tracing::trace!(topic = "a2a_jsonrpc", detail = %log_msg, "JSON-RPC error response");
                },
            }
        }

        let data = match serde_json::to_value([&self.error_info]) {
            Ok(value) => Some(value),
            Err(e) => {
                tracing::error!(topic = "a2a_jsonrpc", error = %e, "Failed to serialise ErrorInfo");
                None
            },
        };

        JsonRpcError {
            code: self.code,
            message: self.message,
            data,
        }
    }

    pub const fn http_status(&self) -> StatusCode {
        http_status_for(self.code)
    }

    pub fn build_with_status(
        self,
        request_id: &NumberOrString,
    ) -> (StatusCode, JsonRpcErrorResponse) {
        let status = self.http_status();
        (status, self.build(request_id))
    }

    pub fn invalid_request() -> Self {
        Self::new(
            INVALID_REQUEST,
            "Invalid Request",
            A2aErrorReason::InvalidRequest,
        )
    }

    pub fn method_not_found() -> Self {
        Self::new(
            METHOD_NOT_FOUND,
            "Method not found",
            A2aErrorReason::MethodNotFound,
        )
    }

    pub fn invalid_params() -> Self {
        Self::new(
            INVALID_PARAMS,
            "Invalid params",
            A2aErrorReason::InvalidParams,
        )
    }

    pub fn internal_error() -> Self {
        Self::new(INTERNAL_ERROR, "Internal error", A2aErrorReason::Internal)
    }

    pub fn parse_error() -> Self {
        Self::new(PARSE_ERROR, "Parse error", A2aErrorReason::InvalidRequest)
    }

    pub fn task_not_found(task_id: &TaskId) -> Self {
        Self::new(
            TASK_NOT_FOUND,
            "Task not found",
            A2aErrorReason::TaskNotFound,
        )
        .with_metadata("taskId", task_id.as_str())
    }

    pub fn task_not_cancelable(task_id: &TaskId) -> Self {
        Self::new(
            TASK_NOT_CANCELABLE,
            "Task cannot be canceled",
            A2aErrorReason::TaskNotCancelable,
        )
        .with_metadata("taskId", task_id.as_str())
    }

    pub fn push_notification_not_supported() -> Self {
        Self::new(
            PUSH_NOTIFICATION_NOT_SUPPORTED,
            "Push notifications are not supported",
            A2aErrorReason::PushNotificationNotSupported,
        )
    }

    pub fn unsupported_operation() -> Self {
        Self::new(
            UNSUPPORTED_OPERATION,
            "This operation is not supported",
            A2aErrorReason::UnsupportedOperation,
        )
    }

    pub fn extended_agent_card_not_configured() -> Self {
        Self::new(
            EXTENDED_AGENT_CARD_NOT_CONFIGURED,
            "Extended agent card is not configured",
            A2aErrorReason::ExtendedAgentCardNotConfigured,
        )
    }

    pub fn version_not_supported(requested: &str) -> Self {
        Self::new(
            VERSION_NOT_SUPPORTED,
            "A2A protocol version not supported",
            A2aErrorReason::VersionNotSupported,
        )
        .with_metadata("requested", requested)
        .with_metadata("supported", PROTOCOL_VERSION)
    }

    pub fn unauthorized(reason: impl Into<String>) -> Self {
        Self::new(
            INVALID_REQUEST,
            "Unauthorized",
            A2aErrorReason::Unauthenticated,
        )
        .with_metadata("detail", reason)
    }

    pub fn forbidden(reason: impl Into<String>) -> Self {
        Self::new(INVALID_REQUEST, "Forbidden", A2aErrorReason::Forbidden)
            .with_metadata("detail", reason)
    }
}

pub fn unauthorized_response(
    reason: impl Into<String>,
    request_id: &NumberOrString,
) -> (StatusCode, JsonRpcErrorResponse) {
    let reason_str = reason.into();
    (
        StatusCode::UNAUTHORIZED,
        JsonRpcErrorBuilder::unauthorized(&reason_str)
            .log_warn(&reason_str)
            .build(request_id),
    )
}

pub fn forbidden_response(
    reason: impl Into<String>,
    request_id: &NumberOrString,
) -> (StatusCode, JsonRpcErrorResponse) {
    let reason_str = reason.into();
    (
        StatusCode::FORBIDDEN,
        JsonRpcErrorBuilder::forbidden(&reason_str)
            .log_warn(&reason_str)
            .build(request_id),
    )
}

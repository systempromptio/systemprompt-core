//! [`ExtensionError`] trait and HTTP/MCP error wire types.
//!
//! Domain crates implement [`ExtensionError`] on their own typed error
//! enums so the API and MCP layers can render them into responses without
//! introducing a dependency on each domain.

use http::StatusCode;

/// Wire representation of an HTTP-shaped API error.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// Stable error code suitable for `serde` and clients.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Suggested HTTP status code.
    pub status: StatusCode,
}

impl ApiError {
    /// Construct a fully-specified [`ApiError`].
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>, status: StatusCode) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            status,
        }
    }
}

/// Wire representation of an MCP-shaped error response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpErrorData {
    /// Numeric error code as required by MCP.
    pub code: i32,
    /// Human-readable message.
    pub message: String,
    /// Optional structured payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpErrorData {
    /// Construct an [`McpErrorData`] without an optional payload.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Attach a structured payload.
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Cross-cutting error contract every extension error must satisfy.
///
/// Implementors expose a stable error code, an HTTP status mapping, a
/// retry hint, and rendering helpers for the API and MCP transports.
pub trait ExtensionError: std::error::Error + Send + Sync + 'static {
    /// Stable string code identifying the error type.
    fn code(&self) -> &'static str;

    /// HTTP status the API layer should respond with.
    fn status(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    /// Whether the caller should retry the request.
    fn is_retryable(&self) -> bool {
        false
    }

    /// Human-friendly message safe to surface to end users.
    fn user_message(&self) -> String {
        self.to_string()
    }

    /// Render the error for the MCP transport.
    fn to_mcp_error(&self) -> McpErrorData {
        McpErrorData {
            code: i32::from(self.status().as_u16()),
            message: self.user_message(),
            data: Some(serde_json::json!({
                "code": self.code(),
                "retryable": self.is_retryable(),
            })),
        }
    }

    /// Render the error for the HTTP API transport.
    fn to_api_error(&self) -> ApiError {
        ApiError {
            code: self.code().to_string(),
            message: self.user_message(),
            status: self.status(),
        }
    }
}

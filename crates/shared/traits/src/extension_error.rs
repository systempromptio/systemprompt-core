//! `ExtensionError` trait for consistent error handling across extensions.

use axum::http::StatusCode;

/// API error response structure.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// HTTP status code.
    pub status: StatusCode,
}

impl ApiError {
    /// Create a new API error.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>, status: StatusCode) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            status,
        }
    }
}

/// MCP protocol error format.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpErrorData {
    /// Error code (typically HTTP-like).
    pub code: i32,
    /// Error message.
    pub message: String,
    /// Additional error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpErrorData {
    /// Create a new MCP error.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Add data to the error.
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Trait for extension error types to enable consistent error handling.
///
/// This trait provides a unified interface for extension errors that can be
/// converted to both HTTP API responses and MCP protocol errors.
///
/// # Example
///
/// ```rust,ignore
/// use systemprompt_traits::{ExtensionError, ApiError, McpErrorData};
/// use axum::http::StatusCode;
///
/// #[derive(Debug, thiserror::Error)]
/// pub enum MyError {
///     #[error("Resource not found: {0}")]
///     NotFound(String),
///     #[error("Invalid input: {0}")]
///     InvalidInput(String),
/// }
///
/// impl ExtensionError for MyError {
///     fn code(&self) -> &'static str {
///         match self {
///             Self::NotFound(_) => "NOT_FOUND",
///             Self::InvalidInput(_) => "INVALID_INPUT",
///         }
///     }
///
///     fn status(&self) -> StatusCode {
///         match self {
///             Self::NotFound(_) => StatusCode::NOT_FOUND,
///             Self::InvalidInput(_) => StatusCode::BAD_REQUEST,
///         }
///     }
/// }
/// ```
pub trait ExtensionError: std::error::Error + Send + Sync + 'static {
    /// Machine-readable error code (e.g., `CONTENT_NOT_FOUND`).
    fn code(&self) -> &'static str;

    /// HTTP status code for API responses.
    fn status(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    /// Whether this error is transient and operation should be retried.
    fn is_retryable(&self) -> bool {
        false
    }

    /// User-facing message (defaults to Display impl).
    fn user_message(&self) -> String {
        self.to_string()
    }

    /// Convert to MCP protocol error format.
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

    /// Convert to API response error.
    fn to_api_error(&self) -> ApiError {
        ApiError {
            code: self.code().to_string(),
            message: self.user_message(),
            status: self.status(),
        }
    }
}

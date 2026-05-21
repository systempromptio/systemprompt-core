//! [`ExtensionError`] trait and HTTP/MCP error wire types.
//!
//! Domain crates implement [`ExtensionError`] on their own typed error
//! enums so the API and MCP layers can render them into responses without
//! introducing a dependency on each domain.

use http::StatusCode;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub status: StatusCode,
}

impl ApiError {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>, status: StatusCode) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            status,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpErrorData {
    pub code: i32,
    pub message: String,
    // JSON: JSON-RPC 2.0 §5.1 error `data` field — protocol boundary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpErrorData {
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

pub trait ExtensionError: std::error::Error + Send + Sync + 'static {
    fn code(&self) -> &'static str;

    fn status(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    fn is_retryable(&self) -> bool {
        false
    }

    fn user_message(&self) -> String {
        self.to_string()
    }

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

    fn to_api_error(&self) -> ApiError {
        ApiError {
            code: self.code().to_string(),
            message: self.user_message(),
            status: self.status(),
        }
    }
}

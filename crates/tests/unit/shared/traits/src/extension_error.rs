//! Tests for extension_error module types.

use axum::http::StatusCode;
use systemprompt_traits::{ApiError, ExtensionError, McpErrorData};

mod api_error_tests {
    use super::*;

    #[test]
    fn new_creates_api_error() {
        let err = ApiError::new("NOT_FOUND", "Resource not found", StatusCode::NOT_FOUND);

        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(err.message, "Resource not found");
        assert_eq!(err.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn new_accepts_string_types() {
        let err = ApiError::new(
            String::from("BAD_REQUEST"),
            String::from("Invalid input"),
            StatusCode::BAD_REQUEST,
        );

        assert_eq!(err.code, "BAD_REQUEST");
        assert_eq!(err.message, "Invalid input");
    }

    #[test]
    fn api_error_is_clone() {
        let err = ApiError::new("ERROR", "Something happened", StatusCode::INTERNAL_SERVER_ERROR);
        let cloned = err.clone();

        assert_eq!(err.code, cloned.code);
        assert_eq!(err.message, cloned.message);
        assert_eq!(err.status, cloned.status);
    }

    #[test]
    fn api_error_is_debug() {
        let err = ApiError::new("TEST", "Test message", StatusCode::OK);
        let debug_str = format!("{:?}", err);

        assert!(debug_str.contains("TEST"));
        assert!(debug_str.contains("Test message"));
    }
}

mod mcp_error_data_tests {
    use super::*;

    #[test]
    fn new_creates_mcp_error() {
        let err = McpErrorData::new(404, "Not found");

        assert_eq!(err.code, 404);
        assert_eq!(err.message, "Not found");
        assert!(err.data.is_none());
    }

    #[test]
    fn with_data_adds_json_data() {
        let err = McpErrorData::new(400, "Bad request")
            .with_data(serde_json::json!({"field": "email", "reason": "invalid format"}));

        assert!(err.data.is_some());
        let data = err.data.unwrap();
        assert_eq!(data["field"], "email");
        assert_eq!(data["reason"], "invalid format");
    }

    #[test]
    fn with_data_is_chainable() {
        let err = McpErrorData::new(500, "Internal error")
            .with_data(serde_json::json!({"trace_id": "abc123"}));

        assert_eq!(err.code, 500);
        assert_eq!(err.message, "Internal error");
        assert!(err.data.is_some());
    }

    #[test]
    fn mcp_error_is_clone() {
        let err = McpErrorData::new(403, "Forbidden")
            .with_data(serde_json::json!({"required_role": "admin"}));
        let cloned = err.clone();

        assert_eq!(err.code, cloned.code);
        assert_eq!(err.message, cloned.message);
        assert_eq!(err.data, cloned.data);
    }

    #[test]
    fn mcp_error_serializes_to_json() {
        let err = McpErrorData::new(401, "Unauthorized");
        let json = serde_json::to_string(&err).unwrap();

        assert!(json.contains("401"));
        assert!(json.contains("Unauthorized"));
    }

    #[test]
    fn mcp_error_deserializes_from_json() {
        let json = r#"{"code": 200, "message": "OK"}"#;
        let err: McpErrorData = serde_json::from_str(json).unwrap();

        assert_eq!(err.code, 200);
        assert_eq!(err.message, "OK");
        assert!(err.data.is_none());
    }

    #[test]
    fn mcp_error_with_data_serializes() {
        let err = McpErrorData::new(422, "Validation failed")
            .with_data(serde_json::json!({"errors": ["field1", "field2"]}));
        let json = serde_json::to_string(&err).unwrap();

        assert!(json.contains("422"));
        assert!(json.contains("Validation failed"));
        assert!(json.contains("errors"));
    }

    #[test]
    fn mcp_error_skips_none_data_in_serialization() {
        let err = McpErrorData::new(200, "OK");
        let json = serde_json::to_string(&err).unwrap();

        // data field should be omitted when None
        assert!(!json.contains("data"));
    }
}

mod extension_error_trait_tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("test error: {0}")]
    struct TestError(String);

    impl ExtensionError for TestError {
        fn code(&self) -> &'static str {
            "TEST_ERROR"
        }

        fn status(&self) -> StatusCode {
            StatusCode::BAD_REQUEST
        }

        fn is_retryable(&self) -> bool {
            true
        }
    }

    #[derive(Debug, thiserror::Error)]
    #[error("default error")]
    struct DefaultError;

    impl ExtensionError for DefaultError {
        fn code(&self) -> &'static str {
            "DEFAULT_ERROR"
        }
    }

    #[test]
    fn code_returns_error_code() {
        let err = TestError("something".to_string());
        assert_eq!(err.code(), "TEST_ERROR");
    }

    #[test]
    fn status_returns_custom_status() {
        let err = TestError("bad input".to_string());
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn status_defaults_to_internal_server_error() {
        let err = DefaultError;
        assert_eq!(err.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn is_retryable_returns_custom_value() {
        let err = TestError("transient".to_string());
        assert!(err.is_retryable());
    }

    #[test]
    fn is_retryable_defaults_to_false() {
        let err = DefaultError;
        assert!(!err.is_retryable());
    }

    #[test]
    fn user_message_returns_display_string() {
        let err = TestError("specific message".to_string());
        assert_eq!(err.user_message(), "test error: specific message");
    }

    #[test]
    fn to_api_error_converts_correctly() {
        let err = TestError("api test".to_string());
        let api_err = err.to_api_error();

        assert_eq!(api_err.code, "TEST_ERROR");
        assert_eq!(api_err.message, "test error: api test");
        assert_eq!(api_err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn to_mcp_error_converts_correctly() {
        let err = TestError("mcp test".to_string());
        let mcp_err = err.to_mcp_error();

        assert_eq!(mcp_err.code, 400); // BAD_REQUEST
        assert_eq!(mcp_err.message, "test error: mcp test");
        assert!(mcp_err.data.is_some());

        let data = mcp_err.data.unwrap();
        assert_eq!(data["code"], "TEST_ERROR");
        assert_eq!(data["retryable"], true);
    }

    #[test]
    fn to_mcp_error_includes_retryable_false() {
        let err = DefaultError;
        let mcp_err = err.to_mcp_error();

        let data = mcp_err.data.unwrap();
        assert_eq!(data["retryable"], false);
    }
}

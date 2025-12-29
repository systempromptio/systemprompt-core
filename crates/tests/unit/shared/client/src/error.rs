//! Unit tests for ClientError
//!
//! Tests cover:
//! - Error creation and variants
//! - from_response constructor
//! - is_retryable method
//! - Display implementations

#[cfg(test)]
use systemprompt_client::{ClientError, ClientResult};

// ============================================================================
// ClientError::from_response Tests
// ============================================================================

#[test]
fn test_from_response_creates_api_error() {
    let error = ClientError::from_response(404, "Not found".to_string());

    match error {
        ClientError::ApiError {
            status,
            message,
            details,
        } => {
            assert_eq!(status, 404);
            assert_eq!(message, "Not found");
            assert_eq!(details, Some("Not found".to_string()));
        }
        _ => panic!("Expected ApiError variant"),
    }
}

#[test]
fn test_from_response_with_empty_body() {
    let error = ClientError::from_response(500, String::new());

    match error {
        ClientError::ApiError {
            status,
            message,
            details,
        } => {
            assert_eq!(status, 500);
            assert!(message.is_empty());
            assert_eq!(details, Some(String::new()));
        }
        _ => panic!("Expected ApiError variant"),
    }
}

#[test]
fn test_from_response_with_json_error_body() {
    let body = r#"{"error": "Invalid request", "code": "INVALID_REQUEST"}"#.to_string();
    let error = ClientError::from_response(400, body.clone());

    match error {
        ClientError::ApiError {
            status,
            message,
            details,
        } => {
            assert_eq!(status, 400);
            assert_eq!(message, body);
            assert_eq!(details, Some(body));
        }
        _ => panic!("Expected ApiError variant"),
    }
}

#[test]
fn test_from_response_various_status_codes() {
    let test_cases = vec![
        (400, "Bad Request"),
        (401, "Unauthorized"),
        (403, "Forbidden"),
        (404, "Not Found"),
        (429, "Too Many Requests"),
        (500, "Internal Server Error"),
        (502, "Bad Gateway"),
        (503, "Service Unavailable"),
    ];

    for (status, message) in test_cases {
        let error = ClientError::from_response(status, message.to_string());
        match error {
            ClientError::ApiError {
                status: s,
                message: m,
                ..
            } => {
                assert_eq!(s, status);
                assert_eq!(m, message);
            }
            _ => panic!("Expected ApiError variant for status {}", status),
        }
    }
}

// ============================================================================
// ClientError::is_retryable Tests
// ============================================================================

#[test]
fn test_timeout_is_retryable() {
    let error = ClientError::Timeout;
    assert!(error.is_retryable());
}

#[test]
fn test_server_unavailable_is_retryable() {
    let error = ClientError::ServerUnavailable("Connection refused".to_string());
    assert!(error.is_retryable());
}

#[test]
fn test_api_error_not_retryable() {
    let error = ClientError::ApiError {
        status: 400,
        message: "Bad request".to_string(),
        details: None,
    };
    assert!(!error.is_retryable());
}

#[test]
fn test_auth_error_not_retryable() {
    let error = ClientError::AuthError("Invalid token".to_string());
    assert!(!error.is_retryable());
}

#[test]
fn test_not_found_not_retryable() {
    let error = ClientError::NotFound("Resource not found".to_string());
    assert!(!error.is_retryable());
}

#[test]
fn test_json_error_not_retryable() {
    let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
    let error = ClientError::JsonError(json_err);
    assert!(!error.is_retryable());
}

#[test]
fn test_config_error_not_retryable() {
    let error = ClientError::ConfigError("Missing config".to_string());
    assert!(!error.is_retryable());
}

// ============================================================================
// ClientError Display Tests
// ============================================================================

#[test]
fn test_timeout_display() {
    let error = ClientError::Timeout;
    assert_eq!(error.to_string(), "Request timeout");
}

#[test]
fn test_auth_error_display() {
    let error = ClientError::AuthError("Token expired".to_string());
    assert_eq!(error.to_string(), "Authentication failed: Token expired");
}

#[test]
fn test_not_found_display() {
    let error = ClientError::NotFound("User 123".to_string());
    assert_eq!(error.to_string(), "Resource not found: User 123");
}

#[test]
fn test_server_unavailable_display() {
    let error = ClientError::ServerUnavailable("Database down".to_string());
    assert_eq!(error.to_string(), "Server unavailable: Database down");
}

#[test]
fn test_config_error_display() {
    let error = ClientError::ConfigError("Invalid URL".to_string());
    assert_eq!(error.to_string(), "Invalid configuration: Invalid URL");
}

#[test]
fn test_api_error_display() {
    let error = ClientError::ApiError {
        status: 503,
        message: "Service temporarily unavailable".to_string(),
        details: Some("Maintenance mode".to_string()),
    };
    assert_eq!(
        error.to_string(),
        "API error: 503 - Service temporarily unavailable"
    );
}

// ============================================================================
// ClientResult Type Tests
// ============================================================================

#[test]
fn test_client_result_ok() {
    let result: ClientResult<i32> = Ok(42);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_client_result_err() {
    let result: ClientResult<i32> = Err(ClientError::Timeout);
    assert!(result.is_err());
}

// ============================================================================
// Error Variant Construction Tests
// ============================================================================

#[test]
fn test_api_error_with_details() {
    let error = ClientError::ApiError {
        status: 422,
        message: "Validation failed".to_string(),
        details: Some("Field 'email' is required".to_string()),
    };

    match error {
        ClientError::ApiError {
            status,
            message,
            details,
        } => {
            assert_eq!(status, 422);
            assert_eq!(message, "Validation failed");
            assert_eq!(details, Some("Field 'email' is required".to_string()));
        }
        _ => panic!("Expected ApiError"),
    }
}

#[test]
fn test_api_error_without_details() {
    let error = ClientError::ApiError {
        status: 500,
        message: "Internal error".to_string(),
        details: None,
    };

    match error {
        ClientError::ApiError { details, .. } => {
            assert!(details.is_none());
        }
        _ => panic!("Expected ApiError"),
    }
}

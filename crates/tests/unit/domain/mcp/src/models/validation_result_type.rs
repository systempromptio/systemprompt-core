//! Unit tests for ValidationResultType enum

use systemprompt_core_mcp::models::ValidationResultType;

// ============================================================================
// ValidationResultType as_str Tests
// ============================================================================

#[test]
fn test_validation_result_type_auth_required_as_str() {
    assert_eq!(ValidationResultType::AuthRequired.as_str(), "auth_required");
}

#[test]
fn test_validation_result_type_port_unavailable_as_str() {
    assert_eq!(
        ValidationResultType::PortUnavailable.as_str(),
        "port_unavailable"
    );
}

#[test]
fn test_validation_result_type_connection_failed_as_str() {
    assert_eq!(
        ValidationResultType::ConnectionFailed.as_str(),
        "connection_failed"
    );
}

#[test]
fn test_validation_result_type_timeout_as_str() {
    assert_eq!(ValidationResultType::Timeout.as_str(), "timeout");
}

#[test]
fn test_validation_result_type_success_as_str() {
    assert_eq!(ValidationResultType::Success.as_str(), "success");
}

#[test]
fn test_validation_result_type_error_as_str() {
    assert_eq!(ValidationResultType::Error.as_str(), "error");
}

// ============================================================================
// ValidationResultType parse Tests
// ============================================================================

#[test]
fn test_validation_result_type_parse_auth_required() {
    assert_eq!(
        ValidationResultType::parse("auth_required"),
        ValidationResultType::AuthRequired
    );
}

#[test]
fn test_validation_result_type_parse_port_unavailable() {
    assert_eq!(
        ValidationResultType::parse("port_unavailable"),
        ValidationResultType::PortUnavailable
    );
}

#[test]
fn test_validation_result_type_parse_connection_failed() {
    assert_eq!(
        ValidationResultType::parse("connection_failed"),
        ValidationResultType::ConnectionFailed
    );
}

#[test]
fn test_validation_result_type_parse_timeout() {
    assert_eq!(
        ValidationResultType::parse("timeout"),
        ValidationResultType::Timeout
    );
}

#[test]
fn test_validation_result_type_parse_success() {
    assert_eq!(
        ValidationResultType::parse("success"),
        ValidationResultType::Success
    );
}

#[test]
fn test_validation_result_type_parse_error() {
    assert_eq!(
        ValidationResultType::parse("error"),
        ValidationResultType::Error
    );
}

#[test]
fn test_validation_result_type_parse_unknown_defaults_to_error() {
    assert_eq!(
        ValidationResultType::parse("unknown"),
        ValidationResultType::Error
    );
    assert_eq!(
        ValidationResultType::parse("invalid"),
        ValidationResultType::Error
    );
    assert_eq!(
        ValidationResultType::parse(""),
        ValidationResultType::Error
    );
    assert_eq!(
        ValidationResultType::parse("random_string"),
        ValidationResultType::Error
    );
}

// ============================================================================
// ValidationResultType Display Tests
// ============================================================================

#[test]
fn test_validation_result_type_display() {
    assert_eq!(
        ValidationResultType::AuthRequired.to_string(),
        "auth_required"
    );
    assert_eq!(
        ValidationResultType::PortUnavailable.to_string(),
        "port_unavailable"
    );
    assert_eq!(
        ValidationResultType::ConnectionFailed.to_string(),
        "connection_failed"
    );
    assert_eq!(ValidationResultType::Timeout.to_string(), "timeout");
    assert_eq!(ValidationResultType::Success.to_string(), "success");
    assert_eq!(ValidationResultType::Error.to_string(), "error");
}

// ============================================================================
// ValidationResultType Equality and Clone Tests
// ============================================================================

#[test]
fn test_validation_result_type_equality() {
    assert_eq!(
        ValidationResultType::AuthRequired,
        ValidationResultType::AuthRequired
    );
    assert_eq!(
        ValidationResultType::Success,
        ValidationResultType::Success
    );
    assert_eq!(ValidationResultType::Error, ValidationResultType::Error);
}

#[test]
fn test_validation_result_type_inequality() {
    assert_ne!(
        ValidationResultType::AuthRequired,
        ValidationResultType::Success
    );
    assert_ne!(ValidationResultType::Timeout, ValidationResultType::Error);
    assert_ne!(
        ValidationResultType::ConnectionFailed,
        ValidationResultType::PortUnavailable
    );
}

#[test]
fn test_validation_result_type_clone() {
    let result = ValidationResultType::Success;
    let cloned = result.clone();
    assert_eq!(result, cloned);
}

#[test]
fn test_validation_result_type_copy() {
    let result = ValidationResultType::Timeout;
    let copied = result;
    assert_eq!(result, copied);
}

// ============================================================================
// ValidationResultType Debug Tests
// ============================================================================

#[test]
fn test_validation_result_type_debug() {
    assert!(format!("{:?}", ValidationResultType::AuthRequired).contains("AuthRequired"));
    assert!(format!("{:?}", ValidationResultType::PortUnavailable).contains("PortUnavailable"));
    assert!(format!("{:?}", ValidationResultType::ConnectionFailed).contains("ConnectionFailed"));
    assert!(format!("{:?}", ValidationResultType::Timeout).contains("Timeout"));
    assert!(format!("{:?}", ValidationResultType::Success).contains("Success"));
    assert!(format!("{:?}", ValidationResultType::Error).contains("Error"));
}

// ============================================================================
// ValidationResultType Roundtrip Tests
// ============================================================================

#[test]
fn test_validation_result_type_roundtrip() {
    let variants = [
        ValidationResultType::AuthRequired,
        ValidationResultType::PortUnavailable,
        ValidationResultType::ConnectionFailed,
        ValidationResultType::Timeout,
        ValidationResultType::Success,
        ValidationResultType::Error,
    ];

    for variant in variants {
        let str_val = variant.as_str();
        let parsed = ValidationResultType::parse(str_val);
        assert_eq!(variant, parsed);
    }
}

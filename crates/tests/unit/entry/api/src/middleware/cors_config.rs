//! Unit tests for CORS middleware configuration
//!
//! Tests cover:
//! - CorsError display implementations
//! - Error variants construction

use systemprompt_api::services::middleware::CorsError;

#[test]
fn invalid_origin_error_display() {
    let err = CorsError::InvalidOrigin {
        origin: "bad\norigin".to_string(),
        reason: "invalid characters".to_string(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("bad\norigin"));
    assert!(msg.contains("invalid characters"));
}

#[test]
fn empty_origins_error_display() {
    let err = CorsError::EmptyOrigins;
    let msg = format!("{}", err);
    assert!(msg.contains("at least one valid origin"));
}

#[test]
fn invalid_origin_error_contains_origin() {
    let err = CorsError::InvalidOrigin {
        origin: "http://example .com".to_string(),
        reason: "contains space".to_string(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("http://example .com"));
}

#[test]
fn cors_error_is_debug() {
    let err = CorsError::EmptyOrigins;
    let debug = format!("{:?}", err);
    assert!(debug.contains("EmptyOrigins"));
}

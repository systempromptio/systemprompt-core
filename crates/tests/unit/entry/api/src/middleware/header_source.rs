//! Unit tests for `HeaderSource` required/optional header extraction.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_api::services::middleware::context::sources::HeaderSource;
use systemprompt_models::execution::ContextExtractionError;

#[test]
fn extract_required_returns_present_value() {
    let mut headers = HeaderMap::new();
    headers.insert("x-context-id", HeaderValue::from_static("ctx-1"));
    let value = HeaderSource::extract_required(&headers, "x-context-id").expect("present");
    assert_eq!(value, "ctx-1");
}

#[test]
fn extract_required_missing_header_errors() {
    let headers = HeaderMap::new();
    let err = HeaderSource::extract_required(&headers, "x-context-id").expect_err("missing");
    assert!(matches!(err, ContextExtractionError::MissingHeader(name) if name == "x-context-id"));
}

#[test]
fn extract_required_non_utf8_value_errors() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-context-id",
        HeaderValue::from_bytes(&[0xff, 0xfe]).expect("opaque header bytes"),
    );
    let err = HeaderSource::extract_required(&headers, "x-context-id").expect_err("invalid");
    assert!(
        matches!(err, ContextExtractionError::InvalidHeaderValue { header, .. } if header == "x-context-id")
    );
}

#[test]
fn extract_optional_returns_value_or_none() {
    let mut headers = HeaderMap::new();
    headers.insert("x-task-id", HeaderValue::from_static("task-1"));
    assert_eq!(
        HeaderSource::extract_optional(&headers, "x-task-id").as_deref(),
        Some("task-1")
    );
    assert!(HeaderSource::extract_optional(&headers, "x-absent").is_none());
}

#[test]
fn extract_optional_non_utf8_value_is_none() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-task-id",
        HeaderValue::from_bytes(&[0xff]).expect("opaque header bytes"),
    );
    assert!(HeaderSource::extract_optional(&headers, "x-task-id").is_none());
}

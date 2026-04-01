//! Tests for HeaderExtractor and HeaderInjectionError

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_security::{HeaderExtractor, HeaderInjectionError};

// ============================================================================
// HeaderExtractor Tests
// ============================================================================

#[test]
fn test_header_extractor_extract_trace_id_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-trace-id", HeaderValue::from_static("my-trace-id-123"));

    let trace_id = HeaderExtractor::extract_trace_id(&headers);
    assert_eq!(trace_id.as_str(), "my-trace-id-123");
}

#[test]
fn test_header_extractor_extract_trace_id_missing() {
    let headers = HeaderMap::new();

    let trace_id = HeaderExtractor::extract_trace_id(&headers);
    assert!(!trace_id.as_str().is_empty(), "Should generate a trace ID when missing");
}

#[test]
fn test_header_extractor_extract_trace_id_generates_unique() {
    let headers = HeaderMap::new();

    let trace_id1 = HeaderExtractor::extract_trace_id(&headers);
    let trace_id2 = HeaderExtractor::extract_trace_id(&headers);

    assert_ne!(
        trace_id1.as_str(),
        trace_id2.as_str(),
        "Generated trace IDs should be unique"
    );
}

#[test]
fn test_header_extractor_extract_context_id_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-context-id", HeaderValue::from_static("my-context-id"));

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert_eq!(context_id.as_str(), "my-context-id");
}

#[test]
fn test_header_extractor_extract_context_id_missing() {
    let headers = HeaderMap::new();

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert!(context_id.as_str().is_empty(), "Should return empty context ID when missing");
}

#[test]
fn test_header_extractor_extract_context_id_empty_value() {
    let mut headers = HeaderMap::new();
    headers.insert("x-context-id", HeaderValue::from_static(""));

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert!(context_id.as_str().is_empty(), "Should return empty for empty header value");
}

#[test]
fn test_header_extractor_extract_task_id_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-task-id", HeaderValue::from_static("task-12345"));

    let task_id = HeaderExtractor::extract_task_id(&headers);
    assert!(task_id.is_some());
    assert_eq!(task_id.unwrap().as_str(), "task-12345");
}

#[test]
fn test_header_extractor_extract_task_id_missing() {
    let headers = HeaderMap::new();

    let task_id = HeaderExtractor::extract_task_id(&headers);
    assert!(task_id.is_none());
}

#[test]
fn test_header_extractor_extract_agent_name_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-agent-name", HeaderValue::from_static("my-custom-agent"));

    let agent_name = HeaderExtractor::extract_agent_name(&headers);
    assert_eq!(agent_name.as_str(), "my-custom-agent");
}

#[test]
fn test_header_extractor_extract_agent_name_missing() {
    let headers = HeaderMap::new();

    let agent_name = HeaderExtractor::extract_agent_name(&headers);
    assert_eq!(agent_name.as_str(), "system", "Should return 'system' when missing");
}

#[test]
fn test_header_extractor_with_all_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("x-trace-id", HeaderValue::from_static("trace-abc"));
    headers.insert("x-context-id", HeaderValue::from_static("context-def"));
    headers.insert("x-task-id", HeaderValue::from_static("task-ghi"));
    headers.insert("x-agent-name", HeaderValue::from_static("agent-jkl"));

    let trace_id = HeaderExtractor::extract_trace_id(&headers);
    let context_id = HeaderExtractor::extract_context_id(&headers);
    let task_id = HeaderExtractor::extract_task_id(&headers);
    let agent_name = HeaderExtractor::extract_agent_name(&headers);

    assert_eq!(trace_id.as_str(), "trace-abc");
    assert_eq!(context_id.as_str(), "context-def");
    assert_eq!(task_id.unwrap().as_str(), "task-ghi");
    assert_eq!(agent_name.as_str(), "agent-jkl");
}

#[test]
fn test_header_extractor_uuid_values() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-trace-id",
        HeaderValue::from_static("550e8400-e29b-41d4-a716-446655440000"),
    );
    headers.insert(
        "x-context-id",
        HeaderValue::from_static("6ba7b810-9dad-11d1-80b4-00c04fd430c8"),
    );

    let trace_id = HeaderExtractor::extract_trace_id(&headers);
    let context_id = HeaderExtractor::extract_context_id(&headers);

    assert_eq!(trace_id.as_str(), "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(context_id.as_str(), "6ba7b810-9dad-11d1-80b4-00c04fd430c8");
}

#[test]
fn test_header_extractor_special_characters() {
    let mut headers = HeaderMap::new();
    headers.insert("x-agent-name", HeaderValue::from_static("agent_with-special.chars"));

    let agent_name = HeaderExtractor::extract_agent_name(&headers);
    assert_eq!(agent_name.as_str(), "agent_with-special.chars");
}

#[test]
fn test_header_extractor_debug() {
    let extractor = HeaderExtractor;
    let debug_str = format!("{:?}", extractor);
    assert!(debug_str.contains("HeaderExtractor"));
}

// ============================================================================
// HeaderInjectionError Tests
// ============================================================================

#[test]
fn test_header_injection_error_display() {
    let error = HeaderInjectionError;
    assert_eq!(
        format!("{}", error),
        "Header value contains invalid characters"
    );
}

#[test]
fn test_header_injection_error_debug() {
    let error = HeaderInjectionError;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("HeaderInjectionError"));
}

#[test]
fn test_header_injection_error_is_std_error() {
    let error: Box<dyn std::error::Error> = Box::new(HeaderInjectionError);
    assert!(error.to_string().contains("invalid characters"));
}

#[test]
fn test_header_injection_error_clone() {
    let error = HeaderInjectionError;
    let cloned = error;
    assert_eq!(format!("{}", error), format!("{}", cloned));
}

#[test]
fn test_header_injection_error_copy() {
    let error = HeaderInjectionError;
    let copied: HeaderInjectionError = error;
    assert_eq!(format!("{}", error), format!("{}", copied));
}

#[test]
fn test_header_injection_error_equality() {
    let error1 = HeaderInjectionError;
    let error2 = HeaderInjectionError;
    assert_eq!(error1, error2);
}

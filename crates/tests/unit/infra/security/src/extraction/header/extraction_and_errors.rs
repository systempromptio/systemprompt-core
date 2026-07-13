//! Tests for HeaderExtractor and HeaderInjectionError

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_security::{HeaderExtractor, HeaderInjectionError};

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
    assert!(
        !trace_id.as_str().is_empty(),
        "Should generate a trace ID when missing"
    );
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

const TEST_CTX: &str = "00000000-0000-4000-8000-000000000001";

#[test]
fn test_header_extractor_extract_context_id_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-context-id", HeaderValue::from_static(TEST_CTX));

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert_eq!(
        context_id
            .as_ref()
            .map(systemprompt_identifiers::ContextId::as_str),
        Some(TEST_CTX)
    );
}

#[test]
fn test_header_extractor_extract_context_id_missing() {
    let headers = HeaderMap::new();

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert!(context_id.is_none(), "Should return None when missing");
}

#[test]
fn test_header_extractor_extract_context_id_malformed() {
    let mut headers = HeaderMap::new();
    headers.insert("x-context-id", HeaderValue::from_static("not-a-uuid"));

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert!(
        context_id.is_none(),
        "Should silently drop a malformed UUID"
    );
}

#[test]
fn test_header_extractor_extract_context_id_empty_value() {
    let mut headers = HeaderMap::new();
    headers.insert("x-context-id", HeaderValue::from_static(""));

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert!(context_id.is_none(), "Empty header value rejects as None");
}

#[test]
fn test_header_extractor_extract_task_id_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-task-id", HeaderValue::from_static("task-12345"));

    let task_id =
        HeaderExtractor::extract_task_id(&headers).expect("Should extract task id from header");
    assert_eq!(task_id.as_str(), "task-12345");
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
    assert_eq!(
        agent_name.as_str(),
        "system",
        "Should return 'system' when missing"
    );
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
    assert!(context_id.is_none(), "non-UUID context-id is rejected");
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
    assert_eq!(
        context_id.unwrap().as_str(),
        "6ba7b810-9dad-11d1-80b4-00c04fd430c8"
    );
}

#[test]
fn test_header_extractor_special_characters() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-agent-name",
        HeaderValue::from_static("agent_with-special.chars"),
    );

    let agent_name = HeaderExtractor::extract_agent_name(&headers);
    assert_eq!(agent_name.as_str(), "agent_with-special.chars");
}

#[test]
fn test_header_extractor_debug() {
    let extractor = HeaderExtractor;
    let debug_str = format!("{:?}", extractor);
    assert!(debug_str.contains("HeaderExtractor"));
}

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

mod gateway_and_provider_request_ids {
    use super::*;
    use systemprompt_identifiers::{GatewayConversationId, ProviderRequestId, headers};
    use systemprompt_security::HeaderInjector;

    #[test]
    fn valid_gateway_conversation_id_is_extracted() {
        let mut headers_map = HeaderMap::new();
        headers_map.insert(
            headers::GATEWAY_CONVERSATION_ID,
            HeaderValue::from_static("ctx_0123456789abcdef"),
        );

        let id = HeaderExtractor::extract_gateway_conversation_id(&headers_map)
            .expect("well-formed id extracts");
        assert_eq!(id.as_str(), "ctx_0123456789abcdef");
    }

    #[test]
    fn malformed_or_empty_gateway_conversation_id_is_ignored() {
        let mut malformed = HeaderMap::new();
        malformed.insert(
            headers::GATEWAY_CONVERSATION_ID,
            HeaderValue::from_static("not-a-ctx-id"),
        );
        assert!(HeaderExtractor::extract_gateway_conversation_id(&malformed).is_none());

        let mut empty = HeaderMap::new();
        empty.insert(
            headers::GATEWAY_CONVERSATION_ID,
            HeaderValue::from_static(""),
        );
        assert!(HeaderExtractor::extract_gateway_conversation_id(&empty).is_none());

        assert!(HeaderExtractor::extract_gateway_conversation_id(&HeaderMap::new()).is_none());
    }

    #[test]
    fn provider_request_id_extraction_accepts_valid_and_ignores_empty() {
        let mut headers_map = HeaderMap::new();
        headers_map.insert(
            headers::PROVIDER_REQUEST_ID,
            HeaderValue::from_static("req-123"),
        );
        let id = HeaderExtractor::extract_provider_request_id(&headers_map)
            .expect("well-formed id extracts");
        assert_eq!(id.as_str(), "req-123");

        let mut empty = HeaderMap::new();
        empty.insert(headers::PROVIDER_REQUEST_ID, HeaderValue::from_static(""));
        assert!(HeaderExtractor::extract_provider_request_id(&empty).is_none());
    }

    #[test]
    fn non_ascii_header_values_are_ignored_not_propagated() {
        let mut headers_map = HeaderMap::new();
        headers_map.insert(
            headers::GATEWAY_CONVERSATION_ID,
            HeaderValue::from_bytes(b"ctx_\xff\xfe").expect("opaque bytes are a legal value"),
        );
        assert!(HeaderExtractor::extract_gateway_conversation_id(&headers_map).is_none());
    }

    #[test]
    fn ids_round_trip_through_their_injectors() {
        let mut headers_map = HeaderMap::new();

        let gcid = GatewayConversationId::try_new("ctx_0123456789abcdef").unwrap();
        HeaderInjector::inject_gateway_conversation_id(&mut headers_map, &gcid)
            .expect("inject succeeds");
        assert_eq!(
            HeaderExtractor::extract_gateway_conversation_id(&headers_map),
            Some(gcid)
        );

        let prid = ProviderRequestId::try_new("req-9").unwrap();
        HeaderInjector::inject_provider_request_id(&mut headers_map, &prid)
            .expect("inject succeeds");
        assert_eq!(
            HeaderExtractor::extract_provider_request_id(&headers_map),
            Some(prid)
        );
    }
}

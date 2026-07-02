//! Status-mapping coverage for `map_upstream_error`, the pure helper the
//! gateway message route uses to translate an upstream provider failure into
//! the HTTP status and client-facing message it returns.

use http::StatusCode;
use systemprompt_api::routes::gateway::messages::map_upstream_error;
use systemprompt_api::services::gateway::protocol::outbound::UpstreamError;

fn status_error(status: u16, message: &str) -> UpstreamError {
    UpstreamError::Status {
        provider: "anthropic".to_owned(),
        status,
        message: message.to_owned(),
    }
}

#[test]
fn client_errors_pass_through_with_provider_message() {
    for code in [400_u16, 404, 422] {
        let (status, message) = map_upstream_error(&status_error(code, "bad thing"));
        assert_eq!(status.as_u16(), code);
        assert!(message.contains("anthropic"), "message: {message}");
        assert!(message.contains("bad thing"), "message: {message}");
    }
}

#[test]
fn rate_limit_maps_to_too_many_requests() {
    let (status, message) = map_upstream_error(&status_error(429, "slow down"));
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(message.contains("slow down"), "message: {message}");
}

#[test]
fn timeout_statuses_map_to_gateway_timeout() {
    for code in [408_u16, 504] {
        let (status, _) = map_upstream_error(&status_error(code, "timeout"));
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
    }
}

#[test]
fn server_errors_are_masked_to_bad_gateway() {
    let (status, message) = map_upstream_error(&status_error(500, "internal upstream detail"));
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(message, "upstream provider error");
    assert!(
        !message.contains("internal upstream detail"),
        "server-error detail must not leak to the client"
    );
}

#[test]
fn unknown_status_defaults_to_bad_gateway() {
    let (status, _) = map_upstream_error(&status_error(418, "teapot"));
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

//! Status-mapping policy for upstream gateway failures. A caller-fault status
//! (bad request, unknown model, rate limit) must reach the client unchanged;
//! provider/credential faults must collapse to 502/504 without leaking the
//! upstream's error detail.

use axum::http::StatusCode;
use systemprompt_api::routes::gateway::messages::map_upstream_error;
use systemprompt_api::services::gateway::protocol::outbound::UpstreamError;

fn status(code: u16) -> UpstreamError {
    UpstreamError::Status {
        provider: "openai".to_owned(),
        status: code,
        message: "boom detail".to_owned(),
    }
}

#[test]
fn caller_fault_statuses_pass_through_with_detail() {
    for code in [400u16, 404, 422] {
        let (mapped, msg) = map_upstream_error(&status(code));
        assert_eq!(mapped.as_u16(), code, "status {code} should pass through");
        assert!(
            msg.contains("openai rejected the request"),
            "status {code} message: {msg}"
        );
        assert!(msg.contains("boom detail"), "status {code} message: {msg}");
    }
}

#[test]
fn rate_limit_maps_to_429() {
    let (mapped, _) = map_upstream_error(&status(429));
    assert_eq!(mapped, StatusCode::TOO_MANY_REQUESTS);
}

#[test]
fn upstream_timeouts_map_to_504() {
    for code in [408u16, 504] {
        let (mapped, _) = map_upstream_error(&status(code));
        assert_eq!(mapped, StatusCode::GATEWAY_TIMEOUT, "status {code}");
    }
}

#[test]
fn auth_and_server_faults_collapse_to_502_without_leaking_detail() {
    for code in [401u16, 403, 500, 502, 503] {
        let (mapped, msg) = map_upstream_error(&status(code));
        assert_eq!(mapped, StatusCode::BAD_GATEWAY, "status {code}");
        assert_eq!(msg, "upstream provider error", "status {code}");
        assert!(
            !msg.contains("boom detail"),
            "status {code} must not leak upstream detail"
        );
    }
}

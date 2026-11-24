//! An admission refused at the configured in-flight limit is answered with
//! 429, as a client-retryable rejection, never as a gateway fault.

use axum::http::StatusCode;
use systemprompt_gateway::service::GatewayError;

#[test]
fn a_saturated_gateway_answers_too_many_requests() {
    let error = GatewayError::Saturated { limit: 4096 };
    assert_eq!(error.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(error.to_string().contains("4096"), "{error}");
}

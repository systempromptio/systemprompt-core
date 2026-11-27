//! Every client failure crosses the boundary as a typed `MobileError`.

use systemprompt_client::ClientError;
use systemprompt_mobile::MobileError;

#[test]
fn a_missing_route_is_not_found() {
    let error = MobileError::from(ClientError::from_response(404, "missing".to_owned()));
    assert_eq!(error, MobileError::NotFound);
}

#[test]
fn any_other_status_is_a_refusal_that_carries_it() {
    for status in [400, 401, 403, 429, 500, 503] {
        let error = MobileError::from(ClientError::from_response(status, String::new()));
        assert_eq!(error, MobileError::Refused { status });
    }
}

#[test]
fn a_timeout_is_a_network_failure() {
    assert_eq!(MobileError::from(ClientError::Timeout), MobileError::Network);
}

#[test]
fn an_internal_failure_carries_a_correlation_id_and_no_cause() {
    let cause = serde_json::from_str::<serde_json::Value>("{not json").expect_err("invalid json");
    let error = MobileError::from(ClientError::JsonError(cause));
    let MobileError::Internal { correlation_id } = &error else {
        panic!("expected an internal error, got {error:?}");
    };
    assert_eq!(correlation_id.len(), 36);
    assert_eq!(error.to_string(), format!("internal error {correlation_id}"));
}

#[test]
fn two_internal_failures_have_distinct_correlation_ids() {
    let first = MobileError::from(ClientError::StreamEndedWithoutExitCode);
    let second = MobileError::from(ClientError::StreamEndedWithoutExitCode);
    assert_ne!(first, second);
}

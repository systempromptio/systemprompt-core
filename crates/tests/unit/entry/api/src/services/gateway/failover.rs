//! The failover decision: which upstream errors leave the primary, the
//! attempt order the breakers dictate, and the per-provider breaker registry.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_api::services::gateway::protocol::outbound::UpstreamError;
use systemprompt_api::services::gateway::service::failover::{
    AttemptPlan, FailoverReason, ProviderBreakers, failover_reason, is_failover_status,
    plan_attempts,
};
use systemprompt_models::services::ResilienceSettings;

fn status_error(status: u16) -> anyhow::Error {
    anyhow::Error::new(UpstreamError::Status {
        provider: "anthropic".to_owned(),
        status,
        message: "upstream said no".to_owned(),
        body: bytes::Bytes::new(),
        retry_after: None,
        request_id: None,
    })
}

#[test]
fn capacity_and_server_errors_fail_over() {
    for status in [429, 500, 502, 503, 529, 599] {
        assert!(is_failover_status(status), "{status} should fail over");
    }
}

#[test]
fn client_errors_do_not_fail_over() {
    for status in [400, 401, 403, 404, 413, 422] {
        assert!(!is_failover_status(status), "{status} is about the request");
    }
}

#[test]
fn failover_reason_reads_the_upstream_status() {
    assert_eq!(
        failover_reason(&status_error(503)),
        Some(FailoverReason::Status(503))
    );
    assert_eq!(failover_reason(&status_error(400)), None);
}

#[test]
fn failover_reason_ignores_errors_that_are_not_upstream_failures() {
    let error = anyhow::anyhow!("adapter could not build the body");
    assert_eq!(failover_reason(&error), None);
}

#[tokio::test]
async fn a_transport_failure_fails_over() {
    let source = reqwest::Client::builder()
        .timeout(Duration::from_millis(200))
        .build()
        .expect("client")
        .get("http://127.0.0.1:1/v1/messages")
        .send()
        .await
        .expect_err("nothing listens on port 1");
    let error = anyhow::Error::new(UpstreamError::Transport {
        provider: "anthropic".to_owned(),
        source,
    });
    assert_eq!(failover_reason(&error), Some(FailoverReason::Transport));
    assert_eq!(FailoverReason::Transport.label(), "transport");
}

#[test]
fn reason_labels_are_stable_metric_values() {
    assert_eq!(FailoverReason::CircuitOpen.label(), "circuit_open");
    assert_eq!(FailoverReason::Status(429).label(), "status_429");
}

#[test]
fn no_fallback_means_the_primary_alone() {
    assert_eq!(plan_attempts(false, false, false), AttemptPlan::PrimaryOnly);
    assert_eq!(plan_attempts(false, true, true), AttemptPlan::PrimaryOnly);
}

#[test]
fn healthy_breakers_try_primary_then_fallback() {
    assert_eq!(
        plan_attempts(true, false, false),
        AttemptPlan::PrimaryThenFallback
    );
}

#[test]
fn a_tripped_primary_is_skipped_without_spending_its_retry_budget() {
    assert_eq!(plan_attempts(true, true, false), AttemptPlan::FallbackOnly);
}

#[test]
fn a_tripped_fallback_is_not_tried_after_a_primary_failure() {
    assert_eq!(plan_attempts(true, false, true), AttemptPlan::PrimaryOnly);
}

#[test]
fn two_tripped_breakers_still_send_the_request() {
    assert_eq!(
        plan_attempts(true, true, true),
        AttemptPlan::PrimaryThenFallback
    );
}

#[test]
fn breakers_are_keyed_per_provider_and_shared_across_calls() {
    let breakers = ProviderBreakers::new();
    let settings = ResilienceSettings::default();
    let a = breakers.for_provider("anthropic", &settings);
    let again = breakers.for_provider("anthropic", &settings);
    let b = breakers.for_provider("vertex", &settings);
    assert!(Arc::ptr_eq(&a, &again));
    assert!(!Arc::ptr_eq(&a, &b));
}

#[test]
fn a_provider_trips_after_its_failure_threshold_and_stays_isolated() {
    let breakers = ProviderBreakers::new();
    let settings = ResilienceSettings {
        breaker_failure_threshold: 2,
        breaker_open_cooldown_ms: 60_000,
        ..ResilienceSettings::default()
    };
    let primary = breakers.for_provider("anthropic", &settings);
    let fallback = breakers.for_provider("vertex", &settings);
    primary.acquire().expect("closed breaker admits").failure();
    assert!(!primary.is_open(), "one failure is below the threshold");
    primary.acquire().expect("still closed").failure();
    assert!(primary.is_open(), "the threshold trips the primary");
    assert!(primary.acquire().is_err(), "an open breaker admits nothing");
    assert!(
        fallback.acquire().is_ok(),
        "the fallback's breaker is its own"
    );
    assert_eq!(
        plan_attempts(
            true,
            primary.acquire().is_err(),
            fallback.acquire().is_err()
        ),
        AttemptPlan::FallbackOnly
    );
}

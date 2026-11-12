//! The failover decision: which upstream errors leave a deployment,
//! the attempt order the breakers dictate, and the per-provider breaker
//! registry.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_gateway::protocol::outbound::UpstreamError;
use systemprompt_gateway::service::GatewayError;
use systemprompt_gateway::service::failover::{
    DeploymentLoad, FailoverReason, ProviderBreakers, REGISTRY_CAP, breaker_key, failover_reason,
    is_failover_status, plan_attempts,
};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::ResilienceSettings;
use systemprompt_manifest::services::catalog::ResolvedHop;

fn status_error(status: u16) -> GatewayError {
    GatewayError::Upstream(UpstreamError::Status {
        provider: "anthropic".to_owned(),
        status,
        message: "upstream said no".to_owned(),
        body: Box::new(bytes::Bytes::new()),
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
    let error = GatewayError::internal(
        "outbound request failed",
        std::io::Error::other("adapter could not build the body"),
    );
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
    let error = GatewayError::Upstream(UpstreamError::Transport {
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
fn the_attempt_plan_puts_healthy_deployments_first_and_never_leaves_a_request_unsent() {
    let table: &[(&[bool], &[usize])] = &[
        (&[false], &[0]),
        (&[true], &[0]),
        (&[false, false, false], &[0, 1, 2]),
        (&[false, true, false], &[0, 2]),
        (&[true, false, false], &[1, 2]),
        (&[true, true, false], &[2]),
        (&[false, true, true], &[0]),
        (&[true, true, true], &[0, 1, 2]),
        (&[], &[]),
    ];
    for (tripped, expected) in table {
        assert_eq!(&plan_attempts(tripped), expected, "tripped = {tripped:?}");
    }
}

#[test]
fn breakers_are_keyed_per_provider_and_shared_across_calls() {
    let breakers = ProviderBreakers::new();
    let settings = ResilienceSettings::default();
    let a = breakers.for_endpoint("anthropic", &settings);
    let again = breakers.for_endpoint("anthropic", &settings);
    let b = breakers.for_endpoint("vertex", &settings);
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
    let primary = breakers.for_endpoint("anthropic", &settings);
    let fallback = breakers.for_endpoint("vertex", &settings);
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
        plan_attempts(&[primary.acquire().is_err(), fallback.acquire().is_err()]),
        vec![1]
    );
}

fn billed_hop(kind: systemprompt_manifest::services::catalog::CredentialKind) -> ResolvedHop {
    let mut hop = systemprompt_catalog_fixtures::hop(
        "shared-endpoint",
        systemprompt_wire::WireProtocol::Anthropic,
        "https://upstream.invalid/v1",
        "m",
        "upstream-1",
    );
    hop.credential.kind = kind;
    hop
}

#[test]
fn a_member_paid_hop_is_keyed_per_member_and_a_platform_hop_per_endpoint() {
    use systemprompt_manifest::services::catalog::CredentialKind;
    let (ann, ben) = (UserId::new("ann"), UserId::new("ben"));
    let platform = billed_hop(CredentialKind::Platform);
    assert_eq!(breaker_key(&platform, &ann), breaker_key(&platform, &ben));
    for kind in [CredentialKind::Caller, CredentialKind::Personal] {
        let hop = billed_hop(kind);
        assert_ne!(breaker_key(&hop, &ann), breaker_key(&hop, &ben), "{kind:?}");
        assert_eq!(breaker_key(&hop, &ann), breaker_key(&hop, &ann), "{kind:?}");
        assert_ne!(breaker_key(&hop, &ann), breaker_key(&platform, &ann));
    }
}

#[test]
fn one_members_tripped_breaker_leaves_the_next_member_closed() {
    use systemprompt_manifest::services::catalog::CredentialKind;
    let breakers = ProviderBreakers::new();
    let settings = ResilienceSettings {
        breaker_failure_threshold: 5,
        breaker_open_cooldown_ms: 60_000,
        ..ResilienceSettings::default()
    };
    let hop = billed_hop(CredentialKind::Caller);
    let ann = breakers.for_endpoint(&breaker_key(&hop, &UserId::new("ann")), &settings);
    for _ in 0..5 {
        ann.acquire().expect("closed until the threshold").failure();
    }
    assert!(ann.is_open(), "ann's five 429s open ann's circuit");
    let ben = breakers.for_endpoint(&breaker_key(&hop, &UserId::new("ben")), &settings);
    assert!(!ben.is_open());
    assert!(
        ben.acquire().is_ok(),
        "ben is admitted on the same endpoint"
    );
}

#[test]
fn the_registries_drop_idle_member_entries_past_the_cap() {
    let breakers = ProviderBreakers::new();
    let load = DeploymentLoad::default();
    let settings = ResilienceSettings {
        breaker_failure_threshold: 1,
        breaker_open_cooldown_ms: 60_000,
        ..ResilienceSettings::default()
    };
    let tripped = breakers.for_endpoint("e#tripped", &settings);
    tripped.acquire().expect("closed").failure();
    drop(tripped);
    let held = breakers.for_endpoint("e#held", &settings);
    let busy = load.enter("e#busy");
    for member in 0..REGISTRY_CAP + 8 {
        let key = format!("e#{member}");
        drop(breakers.for_endpoint(&key, &settings));
        drop(load.enter(&key));
    }
    assert!(breakers.len() <= REGISTRY_CAP, "{}", breakers.len());
    assert!(
        breakers.for_endpoint("e#tripped", &settings).is_open(),
        "an open circuit outlives the sweep"
    );
    assert!(Arc::ptr_eq(
        &held,
        &breakers.for_endpoint("e#held", &settings)
    ));
    assert_eq!(load.in_flight("e#busy"), 1, "a counter in use is kept");
    drop(busy);
}

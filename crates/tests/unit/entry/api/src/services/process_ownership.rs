use std::time::Duration;

use chrono::Utc;
use systemprompt_api::services::proxy::service::owner::owner_url;
use systemprompt_api::services::server::lifecycle::process_ownership::{
    LEASE_TTL, OwnershipStep, RENEW_EVERY, ownership_step, renewal_overdue,
};
use systemprompt_identifiers::{InstanceId, LeaseKey};
use systemprompt_traits::Lease;

fn lease_held_by(owner: &str) -> Lease {
    let now = Utc::now();
    Lease {
        key: LeaseKey::new("process-supervisor"),
        owner: InstanceId::new(owner),
        acquired_at: now,
        expires_at: now,
    }
}

#[test]
fn a_standby_worker_that_wins_the_lease_takes_ownership() {
    let me = InstanceId::new("worker-a");
    let lease = lease_held_by("worker-a");
    assert_eq!(
        ownership_step(false, Some(&lease), &me),
        OwnershipStep::Acquired
    );
}

#[test]
fn the_owner_renewing_its_own_lease_keeps_ownership() {
    let me = InstanceId::new("worker-a");
    let lease = lease_held_by("worker-a");
    assert_eq!(
        ownership_step(true, Some(&lease), &me),
        OwnershipStep::Renewed
    );
}

#[test]
fn a_worker_that_loses_the_claim_stands_by() {
    let me = InstanceId::new("worker-b");
    assert_eq!(ownership_step(false, None, &me), OwnershipStep::Standby);
    let lease = lease_held_by("worker-a");
    assert_eq!(
        ownership_step(false, Some(&lease), &me),
        OwnershipStep::Standby
    );
}

#[test]
fn an_owner_whose_claim_fails_has_lost_ownership() {
    let me = InstanceId::new("worker-a");
    assert_eq!(ownership_step(true, None, &me), OwnershipStep::Lost);
    let peer = lease_held_by("worker-b");
    assert_eq!(ownership_step(true, Some(&peer), &me), OwnershipStep::Lost);
}

#[test]
fn renewal_is_overdue_only_when_the_next_tick_could_miss_expiry() {
    assert!(RENEW_EVERY < LEASE_TTL);
    assert!(!renewal_overdue(Duration::ZERO));
    assert!(!renewal_overdue(
        LEASE_TTL - RENEW_EVERY - Duration::from_secs(1)
    ));
    assert!(renewal_overdue(LEASE_TTL - RENEW_EVERY));
    assert!(renewal_overdue(LEASE_TTL));
}

#[test]
fn owner_url_keeps_the_original_path_and_query() {
    assert_eq!(
        owner_url("http://10.0.0.7:8080", "/api/v1/mcp/files/mcp?x=1"),
        "http://10.0.0.7:8080/api/v1/mcp/files/mcp?x=1"
    );
    assert_eq!(
        owner_url("http://10.0.0.7:8080/", "/api/v1/agents/a"),
        "http://10.0.0.7:8080/api/v1/agents/a"
    );
    assert_eq!(
        owner_url("http://worker:8080", "health"),
        "http://worker:8080/health"
    );
}

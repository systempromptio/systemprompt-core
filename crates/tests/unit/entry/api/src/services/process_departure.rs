//! A worker that stops gracefully leaves the reachable set before it releases
//! its leases: once it is gone a surviving peer claims the keys it was
//! preferred for instead of skipping them, and a peer that stands by on such a
//! key retries the claim within [`CLAIM_RETRY_EVERY`] rather than a whole
//! renewal tick.

use systemprompt_api::services::server::lifecycle::process_ownership::{
    CLAIM_RETRY_EVERY, KeyAction, RENEW_EVERY, key_action, next_tick, with_self,
};
use systemprompt_api::services::server::lifecycle::rendezvous::preferred_owner;
use systemprompt_identifiers::{InstanceId, LeaseKey};

fn pair() -> (InstanceId, InstanceId) {
    (
        InstanceId::new("worker-leaving"),
        InstanceId::new("worker-staying"),
    )
}

fn key_preferring(owner: &InstanceId, pool: &[InstanceId]) -> LeaseKey {
    (0..200)
        .map(|i| LeaseKey::new(format!("mcp:service-{i}")))
        .find(|lease| preferred_owner(lease.as_str(), pool) == Some(owner))
        .expect("a key preferring the owner")
}

#[test]
fn a_survivor_skips_a_departing_peers_key_while_the_peer_is_listed() {
    let (leaving, staying) = pair();
    let pool = with_self(vec![leaving.clone()], &staying);
    let lease = key_preferring(&leaving, &pool);
    assert_eq!(
        key_action(&lease, &staying, &pool, &pool, false),
        KeyAction::Skip
    );
}

#[test]
fn a_survivor_claims_the_key_once_the_peer_has_withdrawn_its_presence() {
    let (leaving, staying) = pair();
    let pool = with_self(vec![leaving.clone()], &staying);
    let lease = key_preferring(&leaving, &pool);
    let after = with_self(Vec::new(), &staying);
    assert_eq!(
        key_action(&lease, &staying, &after, &pool, false),
        KeyAction::Claim,
        "the claim does not wait for the peer's presence to go stale"
    );
}

#[test]
fn the_departing_worker_keeps_renewing_what_it_holds_in_its_own_view() {
    let (leaving, staying) = pair();
    let pool = with_self(vec![staying], &leaving);
    let lease = key_preferring(&leaving, &pool);
    assert_eq!(
        key_action(&lease, &leaving, &pool, &pool, true),
        KeyAction::Renew,
        "the holder never hands a key back to itself; it stops and releases on shutdown"
    );
}

#[test]
fn a_standby_on_a_preferred_key_retries_within_the_claim_retry_window() {
    assert_eq!(
        next_tick(std::time::Duration::ZERO, true),
        CLAIM_RETRY_EVERY
    );
    assert_eq!(next_tick(std::time::Duration::ZERO, false), RENEW_EVERY);
    let late = RENEW_EVERY - std::time::Duration::from_secs(1);
    assert_eq!(next_tick(late, true), std::time::Duration::from_secs(1));
    assert_eq!(next_tick(RENEW_EVERY * 2, true), std::time::Duration::ZERO);
    assert!(CLAIM_RETRY_EVERY < RENEW_EVERY);
}

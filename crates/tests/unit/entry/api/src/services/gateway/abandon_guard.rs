//! Unit tests for the dispatch abandon guard's once-only arming rule.

use systemprompt_api::services::gateway::service::abandon::{ABANDONED_REASON, Arming};

#[test]
fn armed_fires_exactly_once() {
    let mut arming = Arming::armed();
    assert!(arming.take());
    assert!(!arming.take());
}

#[test]
fn disarmed_never_fires() {
    let mut arming = Arming::armed();
    arming.disarm();
    assert!(!arming.take());
    assert!(!arming.take());
}

#[test]
fn disarm_after_firing_is_a_no_op() {
    let mut arming = Arming::armed();
    assert!(arming.take());
    arming.disarm();
    assert!(!arming.take());
}

#[test]
fn abandoned_reason_names_the_client() {
    assert!(ABANDONED_REASON.starts_with("client disconnected"));
}

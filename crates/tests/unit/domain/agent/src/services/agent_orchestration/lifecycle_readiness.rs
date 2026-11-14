// The readiness decision for a spawned agent on each host path: listeners
// identified (Linux and macOS with lsof, Windows with netstat), listeners not
// identifiable (no lsof, hidden sockets), and a marker that is not readable
// (Windows).

use systemprompt_agent::services::agent_orchestration::lifecycle::readiness::{
    MarkerCheck, StartupObservation, StartupVerdict, startup_verdict,
};

const CHILD: u32 = 4242;
const OTHER: u32 = 777;

fn observed(
    connected: bool,
    listeners: Option<Vec<u32>>,
    child_alive: bool,
    marker: MarkerCheck,
) -> StartupObservation {
    StartupObservation {
        connected,
        listeners,
        child_alive,
        marker,
    }
}

#[test]
fn a_port_that_refuses_the_connect_is_not_ready() {
    let o = observed(false, Some(vec![CHILD]), true, MarkerCheck::Verified);
    assert_eq!(startup_verdict(CHILD, &o), StartupVerdict::NotYet);
}

#[test]
fn identified_listeners_naming_the_child_are_ready() {
    let o = observed(true, Some(vec![CHILD]), true, MarkerCheck::Verified);
    assert_eq!(startup_verdict(CHILD, &o), StartupVerdict::Ready);
}

#[test]
fn identified_foreign_listener_is_refused_even_while_the_child_is_alive_and_marked() {
    let o = observed(true, Some(vec![OTHER]), true, MarkerCheck::Verified);
    assert_eq!(startup_verdict(CHILD, &o), StartupVerdict::Foreign(OTHER));
}

#[test]
fn identified_child_whose_marker_does_not_match_is_not_ready() {
    let o = observed(true, Some(vec![CHILD]), true, MarkerCheck::Absent);
    assert_eq!(startup_verdict(CHILD, &o), StartupVerdict::NotYet);
}

#[test]
fn unavailable_listener_lookup_with_a_live_marked_child_is_ready_on_linux_and_macos() {
    let o = observed(true, None, true, MarkerCheck::Verified);
    assert_eq!(startup_verdict(CHILD, &o), StartupVerdict::Ready);
}

#[test]
fn hidden_sockets_with_a_live_marked_child_are_ready() {
    let o = observed(true, Some(Vec::new()), true, MarkerCheck::Verified);
    assert_eq!(startup_verdict(CHILD, &o), StartupVerdict::Ready);
}

#[test]
fn unreadable_marker_with_a_live_child_is_ready_on_windows() {
    let o = observed(true, None, true, MarkerCheck::Unavailable);
    assert_eq!(startup_verdict(CHILD, &o), StartupVerdict::Ready);
    let o = observed(true, Some(vec![CHILD]), true, MarkerCheck::Unavailable);
    assert_eq!(startup_verdict(CHILD, &o), StartupVerdict::Ready);
    let o = observed(true, Some(vec![OTHER]), true, MarkerCheck::Unavailable);
    assert_eq!(startup_verdict(CHILD, &o), StartupVerdict::Foreign(OTHER));
}

#[test]
fn unidentifiable_listeners_never_make_a_dead_or_unmarked_child_ready() {
    let dead = observed(true, None, false, MarkerCheck::Verified);
    assert_eq!(startup_verdict(CHILD, &dead), StartupVerdict::NotYet);
    let unmarked = observed(true, None, true, MarkerCheck::Absent);
    assert_eq!(startup_verdict(CHILD, &unmarked), StartupVerdict::NotYet);
}

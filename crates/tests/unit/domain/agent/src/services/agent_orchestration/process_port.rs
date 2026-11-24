// The bind probe the orchestrator uses to decide whether an agent port must be
// reclaimed before a spawn.

use systemprompt_agent::services::agent_orchestration::process;

#[test]
fn is_port_in_use_false_for_an_unbound_port() {
    let port = systemprompt_test_fixtures::ReservedPort::loopback().release_for_child();
    assert!(!process::is_port_in_use(port));
}

#[test]
fn is_port_in_use_true_while_a_listener_holds_the_port() {
    let held = systemprompt_test_fixtures::ReservedPort::loopback();
    let port = held.port();
    assert!(process::is_port_in_use(port));
}

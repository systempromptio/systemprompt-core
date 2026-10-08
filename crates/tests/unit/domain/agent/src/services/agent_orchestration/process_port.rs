// The bind probe the orchestrator uses to decide whether an agent port must be
// reclaimed before a spawn.

use std::net::TcpListener;

use systemprompt_agent::services::agent_orchestration::process;

#[test]
fn is_port_in_use_false_for_an_unbound_port() {
    let port = TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    assert!(!process::is_port_in_use(port));
}

#[test]
fn is_port_in_use_true_while_a_listener_holds_the_port() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    assert!(process::is_port_in_use(port));
}

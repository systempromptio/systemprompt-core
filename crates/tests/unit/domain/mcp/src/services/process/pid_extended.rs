//! Found-branch coverage for the PID/port lookups. The existing `pid` tests
//! only exercise the "nothing there" paths; here we bind a real loopback
//! listener inside this test process so the `/proc` (Linux) or `lsof` scan
//! resolves back to our own PID and the bound port — driving the success
//! branches without spawning any MCP subprocess.

use std::net::TcpListener;

use systemprompt_mcp::services::process::pid::{
    find_pid_by_port, find_process_on_port_with_name, get_port_by_pid, get_process_name_by_pid,
};

fn bind_loopback() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback");
    let port = listener.local_addr().expect("local addr").port();
    (listener, port)
}

#[test]
fn find_pid_by_port_resolves_to_current_process() {
    let (_listener, port) = bind_loopback();
    let found = find_pid_by_port(port).expect("port lookup succeeds");
    assert_eq!(
        found,
        Some(std::process::id()),
        "a listener bound in this process resolves back to our own PID"
    );
}

#[test]
fn get_port_by_pid_finds_bound_port_for_current_process() {
    let (_listener, port) = bind_loopback();
    let found = get_port_by_pid(std::process::id()).expect("pid lookup succeeds");
    assert_eq!(
        found,
        Some(port),
        "our own PID owns the bound loopback port"
    );
}

#[test]
fn find_process_on_port_with_name_matches_our_process_name() {
    let (_listener, port) = bind_loopback();
    let Some(name) = get_process_name_by_pid(std::process::id()) else {
        return;
    };
    let found = find_process_on_port_with_name(port, &name).expect("lookup succeeds");
    assert_eq!(
        found,
        Some(std::process::id()),
        "matching the real process name on the bound port returns our PID"
    );
}

#[test]
fn find_process_on_port_with_name_rejects_mismatched_name() {
    let (_listener, port) = bind_loopback();
    let found = find_process_on_port_with_name(port, "zzz_not_our_process_name_98765")
        .expect("lookup succeeds");
    assert!(
        found.is_none(),
        "a bound port whose owner name differs yields no match"
    );
}

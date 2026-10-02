use std::net::TcpListener;
use std::num::NonZeroU16;

use systemprompt_mcp::services::process::listener::{
    listener_pids, parse_lsof_pids, parse_netstat_listeners,
};

const NETSTAT: &str = "
Active Connections

  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       1004
  TCP    0.0.0.0:445            0.0.0.0:0              LISTENING       4
  TCP    127.0.0.1:5000         0.0.0.0:0              LISTENING       2222
  TCP    [::]:5000              [::]:0                 LISTENING       2222
  TCP    10.0.0.5:51000         93.184.216.34:5000     ESTABLISHED     3333
  TCP    127.0.0.1:5001         127.0.0.1:5000         ESTABLISHED     4444
  TCP    0.0.0.0:6000           0.0.0.0:0              LISTENING       0
";

fn port(raw: u16) -> NonZeroU16 {
    NonZeroU16::new(raw).expect("non-zero test port")
}

#[test]
fn netstat_selects_only_listeners_on_the_local_port() {
    assert_eq!(parse_netstat_listeners(NETSTAT, port(5000)), vec![2222]);
}

#[test]
fn netstat_never_matches_a_foreign_address_port() {
    assert!(parse_netstat_listeners(NETSTAT, port(51000)).is_empty());
}

#[test]
fn netstat_drops_the_idle_process_pid() {
    assert!(parse_netstat_listeners(NETSTAT, port(6000)).is_empty());
}

#[test]
fn lsof_output_is_deduplicated_and_drops_pid_zero() {
    assert_eq!(parse_lsof_pids("42\n0\n42\nnot-a-pid\n7\n"), vec![42, 7]);
}

#[test]
fn lsof_empty_output_is_no_holder() {
    assert!(parse_lsof_pids("").is_empty());
}

#[test]
fn listener_pids_finds_this_process_listening() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let bound = listener.local_addr().expect("addr").port();

    let pids = listener_pids(port(bound)).expect("lookup runs");

    assert_eq!(pids, vec![std::process::id()]);
}

#[test]
fn listener_pids_ignores_a_released_port() {
    let bound = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr").port()
    };

    assert!(listener_pids(port(bound)).expect("lookup runs").is_empty());
}

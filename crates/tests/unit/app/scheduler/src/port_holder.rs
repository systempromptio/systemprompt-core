//! Port-holder lookup and identity: port 0 never yields a PID or a signal, only
//! listening sockets name a holder, and a holder is a peer instance only when
//! it runs this executable and is not this process.

use std::num::NonZeroU16;

use systemprompt_scheduler::services::orchestration::process_cleanup::listener::{
    executable_names_match, parse_lsof_pids, parse_netstat_listeners,
};
use systemprompt_scheduler::{ProcessCleanup, ServiceManagementService};

const NONEXISTENT_PID: u32 = i32::MAX as u32;

fn port(n: u16) -> NonZeroU16 {
    NonZeroU16::new(n).expect("non-zero port")
}

#[tokio::test]
async fn stop_api_by_port_zero_reports_no_listener_and_signals_nothing() {
    let stopped = ServiceManagementService::stop_api_by_port(0, true)
        .await
        .expect("port 0 has no holder to stop");
    assert_eq!(stopped, None);
}

#[test]
fn port_zero_has_no_holder_to_kill_even_for_this_process() {
    assert!(ProcessCleanup::check_port(0).is_none());
    assert!(ProcessCleanup::kill_port(0, std::process::id()).is_empty());
    assert!(ProcessCleanup::kill_port(0, NONEXISTENT_PID).is_empty());
}

#[test]
fn a_port_this_process_listens_on_is_attributed_to_it() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let bound = listener.local_addr().expect("addr").port();
    assert_eq!(
        ProcessCleanup::listener_pids(port(bound)),
        vec![std::process::id()]
    );
    drop(listener);
}

#[test]
fn this_process_is_never_its_own_peer_instance() {
    assert!(!ProcessCleanup::is_peer_instance(std::process::id()));
}

#[test]
fn a_dead_pid_is_not_a_peer_instance() {
    assert!(!ProcessCleanup::is_peer_instance(NONEXISTENT_PID));
}

#[test]
fn lsof_pids_skip_zero_junk_and_duplicates() {
    assert_eq!(
        parse_lsof_pids("0\n4242\nnot-a-pid\n4242\n 77 \n"),
        vec![4242, 77]
    );
    assert!(parse_lsof_pids("").is_empty());
}

#[test]
fn netstat_listeners_match_the_local_column_in_the_listening_state_only() {
    let table = "\
  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:8080           0.0.0.0:0              LISTENING       1111
  TCP    127.0.0.1:50000        127.0.0.1:8080         ESTABLISHED     2222
  TCP    127.0.0.1:8080         127.0.0.1:50001        ESTABLISHED     3333
  TCP    [::]:8080              [::]:0                 LISTENING       1111
  TCP    0.0.0.0:18080          0.0.0.0:0              LISTENING       4444
  TCP    0.0.0.0:8080           0.0.0.0:0              LISTENING       0
";
    assert_eq!(parse_netstat_listeners(table, port(8080)), vec![1111]);
    assert!(parse_netstat_listeners(table, port(50000)).is_empty());
}

#[test]
fn executable_names_match_exactly_or_by_the_truncated_comm_prefix() {
    assert!(executable_names_match("systemprompt", "systemprompt"));
    assert!(executable_names_match(
        "systemprompt-ab",
        "systemprompt-abcdef"
    ));
    assert!(!executable_names_match(
        "systemprompt-a",
        "systemprompt-abcdef"
    ));
    assert!(!executable_names_match("python3", "systemprompt"));
    assert!(!executable_names_match("", "systemprompt"));
}

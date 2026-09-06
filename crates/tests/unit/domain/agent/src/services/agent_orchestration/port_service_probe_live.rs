//! Port and process probes run against real processes and real sockets.
//!
//! These helpers shell out to `lsof` and `ps`. The behaviour that matters is
//! what the orchestrator acts on: whether a port is attributed to the process
//! actually holding it, and whether a PID is identified as one of ours before
//! it is signalled.

use systemprompt_agent::services::agent_orchestration::port_service::{
    find_process_using_port, get_process_info, is_agent_process,
};
use systemprompt_test_fixtures::bind_in_range;

#[test]
fn a_port_this_process_holds_is_attributed_to_this_process() {
    let listener = bind_in_range(45_000..46_000).expect("a free port in range");
    let port = listener.local_addr().expect("bound address").port();

    let found = find_process_using_port(port).expect("lsof probe runs");

    assert_eq!(
        found,
        Some(std::process::id()),
        "the port is held by this test process, so the probe must name it"
    );
    drop(listener);
}

#[test]
fn a_port_nobody_holds_is_reported_as_free() {
    let listener = bind_in_range(46_000..47_000).expect("a free port in range");
    let port = listener.local_addr().expect("bound address").port();
    drop(listener);

    let found = find_process_using_port(port).expect("lsof probe runs");

    assert_eq!(
        found, None,
        "a released port must not be attributed to any process"
    );
}

#[test]
fn process_info_for_this_process_carries_its_own_command_line() {
    let info = get_process_info(std::process::id())
        .expect("ps probe runs")
        .expect("this process is visible to ps");

    assert_eq!(info.pid, std::process::id());
    assert!(
        !info.command.is_empty(),
        "the command line is what identifies the process; it must not be blank"
    );
}

#[test]
fn a_pid_no_process_holds_yields_no_info_rather_than_an_error() {
    let info = get_process_info(u32::MAX).expect("a dead pid is not a probe failure");

    assert!(
        info.is_none(),
        "an absent process must be reported as absent, not fabricated: {info:?}"
    );
}

// Why: this gates killing. Misidentifying the test runner as an agent would
// let the orchestrator signal an unrelated process.
#[test]
fn the_test_runner_is_not_mistaken_for_an_agent_process() {
    let verdict = is_agent_process(std::process::id())
        .expect("a live pid yields a verdict rather than an error");

    assert!(
        !verdict,
        "the test binary is not an agent worker and must not be claimed as one"
    );
}

#[test]
fn a_pid_that_does_not_exist_is_reported_as_an_error_not_as_not_an_agent() {
    let err = is_agent_process(u32::MAX)
        .expect_err("an absent process is an error, not a negative verdict");

    assert!(
        err.contains("No process info found"),
        "the caller must be able to tell 'gone' from 'not ours': {err}"
    );
}

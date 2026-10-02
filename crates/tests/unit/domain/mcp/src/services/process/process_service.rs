//! Unit tests for the `ProcessService` facade over the loader's process
//! supervision: liveness, port holders and the identity-gated stop.

use std::net::TcpListener;

use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::StopOutcome;
use systemprompt_mcp::services::process::ProcessService;

const DEAD_PID: u32 = 4_194_305;

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("addr").port()
}

#[tokio::test]
async fn a_dead_pid_is_not_running() {
    assert!(!ProcessService::is_running(DEAD_PID).await);
}

#[tokio::test]
async fn the_supervisor_itself_never_reads_as_a_running_child() {
    assert!(!ProcessService::is_running(std::process::id()).await);
}

#[tokio::test]
async fn stopping_a_dead_pid_reports_not_running() {
    let outcome = ProcessService::stop(DEAD_PID, &ServiceName::new("any-service"))
        .await
        .expect("a dead pid is not a failure");

    assert_eq!(outcome, StopOutcome::NotRunning);
}

#[tokio::test]
async fn a_free_port_has_no_listener_and_no_owned_holder() {
    let port = free_port();

    assert!(!ProcessService::port_has_listener(port).await.expect("lsof"));
    assert!(
        ProcessService::owned_port_holders(port, &ServiceName::new("absent"))
            .await
            .expect("lsof")
            .is_empty()
    );
}

#[tokio::test]
async fn an_unmarked_listener_is_seen_but_never_owned() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();

    assert!(ProcessService::port_has_listener(port).await.expect("lsof"));
    assert!(
        ProcessService::owned_port_holders(port, &ServiceName::new("files"))
            .await
            .expect("lsof")
            .is_empty(),
        "the test process carries no MCP marker, so it is never an owned holder"
    );
}

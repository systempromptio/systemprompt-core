use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{
    self, ChildKind, StopOutcome, Termination, parse_lsof_pids, parse_netstat_listeners,
};

const GENEROUS_GRACE: Duration = Duration::from_secs(30);
const MARKER_HELPER: &str = "subprocess::supervised_spawn::marker_helper";

fn shell(script: &str) -> Child {
    Command::new("sh")
        .args(["-c", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sh")
}

fn first_line(child: &mut Child) -> String {
    let stdout = child.stdout.as_mut().expect("piped stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("read child stdout");
    line.trim().to_owned()
}

fn sleeper() -> Child {
    Command::new("sleep")
        .arg("30")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sleep")
}

#[tokio::test]
async fn a_child_that_honours_sigterm_is_reported_exited() {
    let child = sleeper();
    let pid = child.id();
    assert!(subprocess::is_running(pid).await);

    let outcome = subprocess::terminate_gracefully(pid, GENEROUS_GRACE).await;

    assert!(matches!(outcome, Ok(Termination::Exited)), "{outcome:?}");
    assert!(!subprocess::is_running(pid).await);
}

#[tokio::test]
async fn a_child_that_ignores_sigterm_is_killed_after_the_grace() {
    let mut child = shell(r#"trap "" TERM; echo ready; exec sleep 30"#);
    assert_eq!(first_line(&mut child), "ready");
    let pid = child.id();

    let outcome = subprocess::terminate_gracefully(pid, Duration::from_millis(300)).await;

    assert!(matches!(outcome, Ok(Termination::Killed)), "{outcome:?}");
    assert!(!subprocess::is_running(pid).await);
}

#[tokio::test]
async fn an_exited_unreaped_child_reads_as_exited_without_waiting_out_the_grace() {
    let mut child = shell("exit 0");
    let mut drained = Vec::new();
    child
        .stdout
        .take()
        .expect("piped stdout")
        .read_to_end(&mut drained)
        .expect("child closed stdout on exit");
    let pid = child.id();

    let outcome = subprocess::terminate_gracefully(pid, GENEROUS_GRACE).await;

    assert!(
        matches!(
            outcome,
            Ok(Termination::AlreadyExited | Termination::Exited)
        ),
        "{outcome:?}"
    );
    assert!(!subprocess::is_running(pid).await);
}

#[tokio::test]
async fn a_group_leader_is_stopped_with_its_group() {
    let mut command = Command::new("sh");
    command
        .args(["-c", "sleep 30 & echo ready; wait"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    subprocess::place_in_own_process_group(&mut command);
    let mut child = command.spawn().expect("spawn group leader");
    assert_eq!(first_line(&mut child), "ready");
    let pid = child.id();
    assert_eq!(subprocess::process_group(pid).await, Some(pid));

    let outcome = subprocess::terminate_group_gracefully(pid, GENEROUS_GRACE).await;

    assert!(matches!(outcome, Ok(Termination::Exited)), "{outcome:?}");
}

#[tokio::test]
async fn owns_rejects_a_live_pid_without_the_marker() {
    let child = sleeper();
    let pid = child.id();
    let service = ServiceName::new("files");

    assert!(!subprocess::owns(pid, ChildKind::Mcp, &service).await);
    assert!(matches!(
        subprocess::stop_owned(pid, ChildKind::Mcp, &service, GENEROUS_GRACE).await,
        Ok(StopOutcome::NotOurs)
    ));
    assert!(
        subprocess::is_running(pid).await,
        "a foreign pid is never signalled"
    );

    assert!(matches!(
        subprocess::terminate_gracefully(pid, GENEROUS_GRACE).await,
        Ok(Termination::Exited)
    ));
}

#[tokio::test]
async fn stop_owned_stops_a_marked_child() {
    let marked = systemprompt_test_fixtures::spawn_marked_child(MARKER_HELPER, "files");
    let pid = marked.pid();
    let service = ServiceName::new("files");

    assert!(subprocess::owns(pid, ChildKind::Mcp, &service).await);
    assert!(!subprocess::owns(pid, ChildKind::Agent, &service).await);
    assert!(!subprocess::owns(pid, ChildKind::Mcp, &ServiceName::new("other")).await);

    let outcome = subprocess::stop_owned(pid, ChildKind::Mcp, &service, GENEROUS_GRACE).await;

    assert!(
        matches!(outcome, Ok(StopOutcome::Stopped(Termination::Exited))),
        "{outcome:?}"
    );
    assert!(!subprocess::is_running(pid).await);
}

#[tokio::test]
async fn stop_owned_reports_a_reaped_pid_as_not_running() {
    let mut child = Command::new("true").spawn().expect("spawn true");
    child.wait().expect("reap true");

    assert!(matches!(
        subprocess::stop_owned(
            child.id(),
            ChildKind::Agent,
            &ServiceName::new("greeter"),
            GENEROUS_GRACE
        )
        .await,
        Ok(StopOutcome::NotRunning)
    ));
}

#[tokio::test]
async fn the_supervisor_and_pid_zero_are_never_signalled() {
    for pid in [0, std::process::id()] {
        assert!(matches!(
            subprocess::terminate_gracefully(pid, GENEROUS_GRACE).await,
            Ok(Termination::AlreadyExited) | Err(_)
        ));
    }
}

#[tokio::test]
async fn pids_listening_on_finds_the_bound_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("local addr").port();

    let holders = subprocess::pids_listening_on(port).await.expect("lsof");

    assert_eq!(holders, vec![std::process::id()]);
}

#[tokio::test]
async fn pids_listening_on_a_free_port_is_empty() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("local addr").port()
    };

    assert_eq!(
        subprocess::pids_listening_on(port).await.expect("lsof"),
        Vec::<u32>::new()
    );
}

#[test]
fn lsof_pids_are_parsed_sorted_and_deduplicated() {
    assert_eq!(parse_lsof_pids("812\n17\n\n812\nnoise\n"), vec![17, 812]);
    assert_eq!(parse_lsof_pids(""), Vec::<u32>::new());
}

#[test]
fn netstat_counts_only_listeners_on_the_exact_port() {
    let table = "\
  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:8080           0.0.0.0:0              LISTENING       4242
  TCP    [::]:8080              [::]:0                 LISTENING       4242
  TCP    127.0.0.1:18080        0.0.0.0:0              LISTENING       99
  TCP    127.0.0.1:51000        127.0.0.1:8080         ESTABLISHED     77
  TCP    127.0.0.1:8080         127.0.0.1:51000        ESTABLISHED     4242
  UDP    0.0.0.0:8080           *:*                                    55
";

    assert_eq!(parse_netstat_listeners(table, 8080), vec![4242]);
    assert_eq!(parse_netstat_listeners(table, 18080), vec![99]);
    assert_eq!(parse_netstat_listeners(table, 51000), Vec::<u32>::new());
}

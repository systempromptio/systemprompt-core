//! PID/port lookups against real sockets and real processes. On Linux these
//! resolve through `/proc/net/tcp` plus an inode scan of `/proc/<pid>/fd`
//! before falling back to `lsof`, so a bound listener in this process is the
//! only way to drive that path end to end.

use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use systemprompt_mcp::services::process::pid::{
    find_pid_by_port, find_pids_by_name, find_process_on_port_with_name, get_port_by_pid,
    get_process_name_by_pid,
};

fn held_port() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

fn free_port() -> u16 {
    let (listener, port) = held_port();
    drop(listener);
    port
}

// A child whose argv carries a unique marker, so `pgrep -f` can single it out
// without matching this test binary or a sibling test's child.
fn spawn_marked_child(marker: &str) -> Option<Child> {
    // The marker has to be the process itself, not a shell wrapper: a wrapper
    // leaves an orphaned `sleep` behind when the test kills it, and passing the
    // marker to `sleep` as an argument makes sleep reject it and exit at once.
    // A uniquely-named symlink to `sleep` puts the marker in argv[0].
    let dir = tempfile::tempdir().ok()?;
    let link = dir.path().join(marker);
    std::os::unix::fs::symlink("/bin/sleep", &link).ok()?;
    let child = Command::new(&link).arg("60").spawn().ok()?;
    std::mem::forget(dir);

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if find_pids_by_name(marker).is_ok_and(|pids| pids.contains(&child.id())) {
            return Some(child);
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let mut child = child;
    let _ = child.kill();
    let _ = child.wait();
    None
}

#[test]
fn find_pid_by_port_resolves_a_listener_to_its_owning_process() {
    let (_listener, port) = held_port();

    let pid = find_pid_by_port(port)
        .expect("lookup succeeds")
        .expect("a bound port has an owner");
    assert_eq!(
        pid,
        std::process::id(),
        "the listener belongs to this process"
    );
}

#[test]
fn find_pid_by_port_reports_none_for_an_unbound_port() {
    assert_eq!(
        find_pid_by_port(free_port()).expect("lookup succeeds"),
        None
    );
}

#[test]
fn get_port_by_pid_reports_a_port_this_process_listens_on() {
    let (_listener, port) = held_port();

    let found = get_port_by_pid(std::process::id())
        .expect("lookup succeeds")
        .expect("this process holds at least one listening socket");
    assert!(
        found >= 1024,
        "the reported port is a real listening port, got {found} (held {port})"
    );
}

#[test]
fn get_port_by_pid_reports_none_for_a_process_that_does_not_exist() {
    assert_eq!(get_port_by_pid(u32::MAX).expect("lookup succeeds"), None);
}

#[test]
fn get_process_name_by_pid_names_this_process_and_rejects_a_dead_pid() {
    let name = get_process_name_by_pid(std::process::id()).expect("this process is inspectable");
    assert!(!name.is_empty());
    assert!(
        !name.contains('/'),
        "`ps -o comm=` yields a bare command name, got {name}"
    );

    assert_eq!(get_process_name_by_pid(u32::MAX), None);
}

#[test]
fn find_pids_by_name_finds_a_marked_child_and_nothing_for_an_absent_pattern() {
    let marker = format!("mcp-pid-marker-{}", uuid::Uuid::new_v4().simple());
    // skip-ok: no spawnable child process on this host
    let Some(mut child) = spawn_marked_child(&marker) else {
        return;
    };

    let pids = find_pids_by_name(&marker).expect("pgrep runs");
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        pids.contains(&child.id()),
        "the marked child {} is not in {pids:?}",
        child.id()
    );

    let absent = format!("mcp-absent-{}", uuid::Uuid::new_v4().simple());
    assert!(
        find_pids_by_name(&absent).expect("pgrep runs").is_empty(),
        "no process matches a freshly-minted pattern"
    );
}

#[test]
fn find_process_on_port_with_name_matches_only_the_expected_command() {
    let (_listener, port) = held_port();
    // skip-ok: no spawnable child process on this host
    let Some(actual_name) = get_process_name_by_pid(std::process::id()) else {
        return;
    };

    assert_eq!(
        find_process_on_port_with_name(port, &actual_name).expect("lookup succeeds"),
        Some(std::process::id()),
        "the holder is returned when the command name matches"
    );

    assert_eq!(
        find_process_on_port_with_name(port, "definitely-not-this-command")
            .expect("lookup succeeds"),
        None,
        "a name mismatch withholds the pid rather than returning the wrong owner"
    );
}

#[test]
fn find_process_on_port_with_name_reports_none_for_an_unbound_port() {
    assert_eq!(
        find_process_on_port_with_name(free_port(), "anything").expect("lookup succeeds"),
        None
    );
}

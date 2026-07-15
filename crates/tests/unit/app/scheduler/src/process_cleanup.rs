//! Tests for ProcessCleanup primitives.
//!
//! Focuses on safety guards (protected ports/processes) and pure return-value
//! paths (lookups against PIDs/ports that do not exist).

use systemprompt_scheduler::ProcessCleanup;

const NONEXISTENT_PID: u32 = i32::MAX as u32;
const POSTGRES_PORT: u16 = 5432;
const PGBOUNCER_PORT: u16 = 6432;
const UNLIKELY_PORT: u16 = 1;

#[test]
fn check_port_returns_none_for_protected_postgres() {
    assert!(ProcessCleanup::check_port(POSTGRES_PORT).is_none());
}

#[test]
fn check_port_returns_none_for_protected_pgbouncer() {
    assert!(ProcessCleanup::check_port(PGBOUNCER_PORT).is_none());
}

#[test]
fn check_port_returns_none_for_unbound_port() {
    assert!(ProcessCleanup::check_port(UNLIKELY_PORT).is_none());
}

#[test]
fn kill_port_protected_returns_empty() {
    assert!(ProcessCleanup::kill_port(POSTGRES_PORT, NONEXISTENT_PID).is_empty());
    assert!(ProcessCleanup::kill_port(PGBOUNCER_PORT, NONEXISTENT_PID).is_empty());
}

#[test]
fn kill_port_unbound_returns_empty() {
    assert!(ProcessCleanup::kill_port(UNLIKELY_PORT, NONEXISTENT_PID).is_empty());
}

#[test]
fn kill_by_pattern_rejects_protected_postgres() {
    assert_eq!(ProcessCleanup::kill_by_pattern("postgres"), 0);
    assert_eq!(ProcessCleanup::kill_by_pattern("pgbouncer"), 0);
    assert_eq!(ProcessCleanup::kill_by_pattern("psql"), 0);
}

#[test]
fn kill_by_pattern_rejects_pattern_containing_protected_substring() {
    assert_eq!(ProcessCleanup::kill_by_pattern("my-postgres-runner"), 0);
}

#[test]
fn kill_by_pattern_rejects_unsafe_characters() {
    assert_eq!(ProcessCleanup::kill_by_pattern("foo; rm -rf /"), 0);
    assert_eq!(ProcessCleanup::kill_by_pattern("foo$bar"), 0);
    assert_eq!(ProcessCleanup::kill_by_pattern(""), 0);
}

#[test]
fn kill_by_pattern_accepts_safe_nonexistent_pattern() {
    // Safe pattern that should never match any real process; pkill returns
    // non-zero status which the helper maps to 0.
    let unique = format!("systemprompt-test-no-match-{}", std::process::id());
    assert_eq!(ProcessCleanup::kill_by_pattern(&unique), 0);
}

#[test]
fn process_exists_false_for_nonexistent_pid() {
    assert!(!ProcessCleanup::process_exists(NONEXISTENT_PID));
}

#[test]
fn process_exists_true_for_current_pid() {
    assert!(ProcessCleanup::process_exists(std::process::id()));
}

#[test]
fn kill_process_false_for_nonexistent_pid() {
    assert!(!ProcessCleanup::kill_process(NONEXISTENT_PID));
}

#[test]
fn get_process_by_port_returns_none_for_unbound_port() {
    assert!(ProcessCleanup::get_process_by_port(UNLIKELY_PORT).is_none());
}

#[tokio::test]
async fn wait_for_port_free_returns_ok_when_unbound() {
    ProcessCleanup::wait_for_port_free(UNLIKELY_PORT, 1, 1)
        .await
        .expect("unbound port reported free");
}

#[tokio::test]
async fn wait_for_port_free_returns_ok_for_protected_port() {
    // Protected ports are reported as "not occupied" by check_port, so the
    // wait loop exits with Ok on the first iteration.
    ProcessCleanup::wait_for_port_free(POSTGRES_PORT, 2, 1)
        .await
        .expect("protected port reported free");
}

#[tokio::test]
async fn terminate_gracefully_false_for_nonexistent_pid() {
    assert!(!ProcessCleanup::terminate_gracefully(NONEXISTENT_PID, 1).await);
}

#[cfg(unix)]
mod kill_port_ownership {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[test]
    fn port_held_by_a_foreign_process_is_left_untouched() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();

        // The holder is this test process; the claimed owner (PID 1) is
        // neither the holder nor its process group, so nothing is killed.
        let killed = ProcessCleanup::kill_port(port, 1);
        assert!(killed.is_empty(), "a mismatched owner must never be killed");
        assert_eq!(
            ProcessCleanup::check_port(port),
            Some(std::process::id()),
            "the listener must still be alive and holding the port"
        );
    }

    #[test]
    fn port_held_by_the_owning_pid_is_killed() {
        let mut child = Command::new("python3")
            .args([
                "-c",
                "import socket,sys,time\ns=socket.socket()\ns.bind(('127.0.0.1',0))\nprint(s.getsockname()[1],flush=True)\ns.listen(1)\ntime.sleep(60)",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn python3 port holder");
        let stdout = child.stdout.take().expect("holder stdout");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("read holder port");
        let port = line.trim().parse::<u16>().expect("holder port number");
        let pid = child.id();

        let killed = ProcessCleanup::kill_port(port, pid);
        assert_eq!(killed, vec![pid], "the owning holder must be killed");

        // A killed child is a zombie until reaped, so death is observed via
        // try_wait rather than process_exists.
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().expect("try_wait").is_none() {
            assert!(Instant::now() < deadline, "killed holder must die");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

#[cfg(unix)]
mod wait_for_port_free_occupied {
    use super::*;
    use std::net::TcpListener;

    #[tokio::test]
    async fn occupied_port_errors_naming_the_holder() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();

        let err = ProcessCleanup::wait_for_port_free(port, 2, 10)
            .await
            .expect_err("an occupied port must not be reported free");
        let msg = err.to_string();
        assert!(
            msg.contains(&format!("Port {port} still occupied by PID")),
            "the error must name the port and holding PID, got: {msg}"
        );
    }
}

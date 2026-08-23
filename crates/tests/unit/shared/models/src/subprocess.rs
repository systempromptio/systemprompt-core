use systemprompt_models::subprocess::{
    AGENT_NAME_ENV, MCP_SERVICE_ID_ENV, environ_from_procargs2, environ_identifies_child,
    live_pid_is_subprocess,
};

fn environ(vars: &[&str]) -> Vec<u8> {
    let mut blob = Vec::new();
    for v in vars {
        blob.extend_from_slice(v.as_bytes());
        blob.push(0);
    }
    blob
}

#[test]
fn matches_agent_child_with_marker_and_name() {
    let env = environ(&[
        "PATH=/usr/bin",
        "SYSTEMPROMPT_SUBPROCESS=1",
        "AGENT_NAME=greeter",
    ]);
    assert!(environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}

#[test]
fn matches_mcp_child_with_marker_and_name() {
    let env = environ(&["SYSTEMPROMPT_SUBPROCESS=1", "MCP_SERVICE_ID=files"]);
    assert!(environ_identifies_child(&env, MCP_SERVICE_ID_ENV, "files"));
}

#[test]
fn rejects_missing_subprocess_marker() {
    let env = environ(&["AGENT_NAME=greeter"]);
    assert!(!environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}

#[test]
fn rejects_marker_with_wrong_name() {
    let env = environ(&["SYSTEMPROMPT_SUBPROCESS=1", "AGENT_NAME=other"]);
    assert!(!environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}

#[test]
fn rejects_name_as_substring() {
    let env = environ(&["SYSTEMPROMPT_SUBPROCESS=1", "AGENT_NAME=greeter-staging"]);
    assert!(!environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}

#[test]
fn rejects_empty_environ() {
    assert!(!environ_identifies_child(&[], AGENT_NAME_ENV, "greeter"));
}

#[test]
fn rejects_unrelated_process() {
    let env = environ(&["PATH=/usr/bin", "HOME=/root", "TERM=xterm"]);
    assert!(!environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}

#[test]
fn live_pid_without_proc_entry_is_not_our_child() {
    assert!(!live_pid_is_subprocess(
        4_000_000,
        AGENT_NAME_ENV,
        "greeter"
    ));
}

#[test]
fn live_pid_self_is_not_claimed() {
    let me = std::process::id();
    assert!(!live_pid_is_subprocess(me, AGENT_NAME_ENV, "greeter"));
}

mod procargs2 {
    //! `environ_from_procargs2` parses the Darwin `KERN_PROCARGS2` blob:
    //! `argc`, the exec path, NUL padding, `argc` argv entries, then the
    //! environment. The parse is pure, so it is exercised on every platform —
    //! the blobs below are the shapes the kernel produces plus the malformed
    //! ones a recycled or hostile pid could hand back.

    use super::{MCP_SERVICE_ID_ENV, environ_from_procargs2, environ_identifies_child};

    fn blob(argc: i32, exec_path: &str, pad: usize, argv: &[&str], env: &[&str]) -> Vec<u8> {
        let mut out = argc.to_ne_bytes().to_vec();
        out.extend_from_slice(exec_path.as_bytes());
        out.push(0);
        out.extend(std::iter::repeat_n(0u8, pad));
        for entry in argv.iter().chain(env.iter()) {
            out.extend_from_slice(entry.as_bytes());
            out.push(0);
        }
        out
    }

    #[test]
    fn returns_only_the_environment_block() {
        let raw = blob(
            2,
            "/opt/bin/mcp",
            7,
            &["mcp", "--serve"],
            &["SYSTEMPROMPT_SUBPROCESS=1", "MCP_SERVICE_ID=files"],
        );

        let environ = environ_from_procargs2(&raw).expect("well-formed blob parses");

        assert!(environ_identifies_child(
            environ,
            MCP_SERVICE_ID_ENV,
            "files"
        ));
    }

    #[test]
    fn argv_is_skipped_by_count_not_searched_past() {
        // A command line may carry text that is byte-identical to a marker
        // pair. Counting past argv is what stops `env MCP_SERVICE_ID=files ...`
        // from reading as a marked environment and getting an unrelated
        // process signalled.
        let raw = blob(
            3,
            "/usr/bin/env",
            1,
            &["env", "SYSTEMPROMPT_SUBPROCESS=1", "MCP_SERVICE_ID=files"],
            &["PATH=/usr/bin"],
        );

        let environ = environ_from_procargs2(&raw).expect("well-formed blob parses");

        assert!(!environ_identifies_child(
            environ,
            MCP_SERVICE_ID_ENV,
            "files"
        ));
    }

    #[test]
    fn zero_argc_yields_the_whole_environment() {
        let raw = blob(0, "/opt/bin/mcp", 3, &[], &["MCP_SERVICE_ID=files"]);

        assert_eq!(
            environ_from_procargs2(&raw),
            Some(b"MCP_SERVICE_ID=files\0".as_slice())
        );
    }

    #[test]
    fn rejects_argc_larger_than_the_entries_present() {
        let raw = blob(9, "/opt/bin/mcp", 1, &["mcp"], &["MCP_SERVICE_ID=files"]);

        assert_eq!(environ_from_procargs2(&raw), None);
    }

    #[test]
    fn rejects_a_blob_truncated_before_the_exec_path_ends() {
        let mut raw = 1i32.to_ne_bytes().to_vec();
        raw.extend_from_slice(b"/opt/bin/mcp");

        assert_eq!(environ_from_procargs2(&raw), None);
    }

    #[test]
    fn rejects_a_blob_too_short_to_hold_argc() {
        assert_eq!(environ_from_procargs2(&[0, 0]), None);
    }

    #[test]
    fn rejects_a_negative_argc() {
        let raw = blob(-1, "/opt/bin/mcp", 1, &[], &["MCP_SERVICE_ID=files"]);

        assert_eq!(environ_from_procargs2(&raw), None);
    }
}

mod supervised_spawn {
    //! `spawn_supervised` arms `PR_SET_PDEATHSIG` in the forked child. Whether
    //! the kernel actually delivers that signal cannot be asserted from inside
    //! the supervisor — it fires on *this* process dying — so these cover the
    //! parts that are observable: the spawn succeeds, the child survives the
    //! `pre_exec` hook rather than being killed by its own race check, and
    //! concurrent callers are serialised onto the one long-lived spawner thread
    //! that makes the signal meaningful.

    use super::{MCP_SERVICE_ID_ENV, live_pid_is_subprocess};
    use std::process::Command;
    use std::time::{Duration, Instant};
    use systemprompt_models::subprocess::spawn_supervised;

    fn sleeper() -> Command {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        cmd
    }

    fn parent_of(pid: u32) -> Option<u32> {
        let out = Command::new("ps")
            .args(["-o", "ppid=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }

    fn alive(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|s| s.success())
    }

    fn kill(pid: u32) {
        Command::new("kill")
            .arg("-9")
            .arg(pid.to_string())
            .status()
            .ok();
    }

    #[test]
    fn spawned_child_survives_the_pre_exec_hook() {
        let pid = spawn_supervised(sleeper()).expect("spawn");

        // The race check `_exit(0)`s the child when the supervisor has already
        // died. A bug there would kill every child instantly, so give it a
        // moment and confirm it is still running.
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline && !alive(pid) {
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(alive(pid), "child was killed by its own pre_exec hook");
        assert_eq!(
            parent_of(pid),
            Some(std::process::id()),
            "child must be parented to the supervisor, not reparented to init"
        );

        kill(pid);
    }

    #[test]
    fn concurrent_callers_are_serialised_onto_one_spawner() {
        let handles: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(|| spawn_supervised(sleeper()).expect("spawn")))
            .collect();

        let pids: Vec<u32> = handles
            .into_iter()
            .map(|h| h.join().expect("spawn thread"))
            .collect();

        for pid in &pids {
            assert!(alive(*pid), "pid {pid} did not survive a concurrent spawn");
            assert_eq!(
                parent_of(*pid),
                Some(std::process::id()),
                "every child is parented to the supervisor regardless of caller thread"
            );
        }

        for pid in pids {
            kill(pid);
        }
    }

    #[test]
    fn spawn_failure_is_reported_not_panicked() {
        // The spawn happens on a shared worker thread; a failure there must come
        // back to this caller rather than poisoning the thread for everyone else.
        spawn_supervised(Command::new("definitely-not-a-real-binary-9f2a"))
            .expect_err("missing binary must surface as an error");

        let pid = spawn_supervised(sleeper()).expect("spawner still serves later callers");
        assert!(alive(pid));
        kill(pid);
    }

    const MARKER_HELPER: &str = "subprocess::supervised_spawn::marker_helper";

    #[test]
    #[ignore = "re-executed as a child process by the identity tests below"]
    fn marker_helper() {
        systemprompt_test_fixtures::announce_helper_ready();
        std::thread::sleep(Duration::from_secs(30));
    }

    #[test]
    fn a_spawned_child_verifies_as_ours_by_its_markers() {
        // The whole reclaim path — port cleanup, shutdown, reconciliation —
        // hangs off `live_pid_is_subprocess` recognising a live child by the
        // markers the supervisor stamped. Reading them back off a real process
        // is the only assertion that proves the platform backend works.
        let child = systemprompt_test_fixtures::spawn_marked_child(MARKER_HELPER, "files");

        assert!(
            live_pid_is_subprocess(child.pid(), MCP_SERVICE_ID_ENV, "files"),
            "a child carrying our markers must verify as ours"
        );
        assert!(
            !live_pid_is_subprocess(child.pid(), MCP_SERVICE_ID_ENV, "other"),
            "a different service name must not claim this child"
        );
    }

    #[test]
    fn an_unmarked_child_is_never_claimed() {
        let pid = spawn_supervised(sleeper()).expect("spawn");

        assert!(alive(pid));
        assert!(!live_pid_is_subprocess(pid, MCP_SERVICE_ID_ENV, "files"));

        kill(pid);
    }
}

use systemprompt_models::subprocess::{
    AGENT_NAME_ENV, MCP_SERVICE_ID_ENV, environ_identifies_child, live_pid_is_subprocess,
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

mod supervised_spawn {
    //! `spawn_supervised` arms `PR_SET_PDEATHSIG` in the forked child. Whether
    //! the kernel actually delivers that signal cannot be asserted from inside
    //! the supervisor — it fires on *this* process dying — so these cover the
    //! parts that are observable: the spawn succeeds, the child survives the
    //! `pre_exec` hook rather than being killed by its own race check, and
    //! concurrent callers are serialised onto the one long-lived spawner thread
    //! that makes the signal meaningful.

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
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let after_comm = stat.rsplit_once(')')?.1;
        after_comm.split_whitespace().nth(1)?.parse().ok()
    }

    fn alive(pid: u32) -> bool {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
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
}

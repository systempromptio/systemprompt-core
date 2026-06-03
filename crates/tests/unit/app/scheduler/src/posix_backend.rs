//! Exercises the POSIX process-cleanup backend through the public
//! [`ProcessCleanup`] surface, covering the PID-liveness / signalable-PID
//! decision paths without ever signalling a real, unrelated process.
//!
//! PID 0 and out-of-range PIDs route through `signalable_pid`, which rejects
//! them as un-signalable (they would otherwise reach a process group), so
//! every primitive must treat them as dead / no-op. The current process PID is
//! the only PID these tests ever confirm as alive.

use systemprompt_scheduler::ProcessCleanup;

// PID 0 is rejected by signalable_pid (it means "the caller's own group").
const PID_ZERO: u32 = 0;
// Above i32::MAX wraps to a negative i32, which kill(2) reads as a group.
const OUT_OF_RANGE_PID: u32 = (i32::MAX as u32) + 1;
const NONEXISTENT_PID: u32 = i32::MAX as u32;

mod process_exists_guards {
    use super::*;

    #[test]
    fn pid_zero_does_not_exist() {
        assert!(!ProcessCleanup::process_exists(PID_ZERO));
    }

    #[test]
    fn out_of_range_pid_does_not_exist() {
        assert!(!ProcessCleanup::process_exists(OUT_OF_RANGE_PID));
    }

    #[test]
    fn current_pid_exists() {
        assert!(ProcessCleanup::process_exists(std::process::id()));
    }
}

mod kill_process_guards {
    use super::*;

    #[test]
    fn pid_zero_is_not_killed() {
        assert!(!ProcessCleanup::kill_process(PID_ZERO));
    }

    #[test]
    fn out_of_range_pid_is_not_killed() {
        assert!(!ProcessCleanup::kill_process(OUT_OF_RANGE_PID));
    }

    #[test]
    fn nonexistent_pid_is_not_killed() {
        assert!(!ProcessCleanup::kill_process(NONEXISTENT_PID));
    }
}

mod terminate_gracefully_guards {
    use super::*;

    #[tokio::test]
    async fn pid_zero_terminates_to_false() {
        assert!(!ProcessCleanup::terminate_gracefully(PID_ZERO, 1).await);
    }

    #[tokio::test]
    async fn out_of_range_pid_terminates_to_false() {
        assert!(!ProcessCleanup::terminate_gracefully(OUT_OF_RANGE_PID, 1).await);
    }

    #[tokio::test]
    async fn nonexistent_pid_terminates_to_false() {
        assert!(!ProcessCleanup::terminate_gracefully(NONEXISTENT_PID, 1).await);
    }
}

mod terminate_group_gracefully_guards {
    use super::*;

    #[tokio::test]
    async fn pid_zero_group_terminates_to_false() {
        // Un-signalable id: falls through to the early `None` guard, never
        // broadcasting to a group.
        assert!(!ProcessCleanup::terminate_group_gracefully(PID_ZERO, 1).await);
    }

    #[tokio::test]
    async fn out_of_range_pid_group_terminates_to_false() {
        assert!(!ProcessCleanup::terminate_group_gracefully(OUT_OF_RANGE_PID, 1).await);
    }

    #[tokio::test]
    async fn nonexistent_pid_group_terminates_to_false() {
        // A non-existent (but signalable) PID is not its own group leader, so
        // the group path falls back to single-PID termination, which fails.
        assert!(!ProcessCleanup::terminate_group_gracefully(NONEXISTENT_PID, 1).await);
    }
}

// Real-child termination: a killed child the test never reaps becomes a zombie
// that still answers `kill(pid, 0)`, mirroring the supervisor's forgotten
// children. These confirm the grace poll is zombie-aware (returns as soon as
// the child exits) rather than always sleeping the full grace window.
#[cfg(unix)]
mod live_child_termination {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command};
    use std::time::{Duration, Instant};

    fn spawn_in_own_group(program: &str, args: &[&str]) -> Child {
        let mut command = Command::new(program);
        command.args(args).process_group(0);
        command.spawn().expect("spawn test child")
    }

    #[tokio::test]
    async fn returns_early_when_child_exits_on_sigterm() {
        let mut child = spawn_in_own_group("sleep", &["30"]);
        let pid = child.id();

        let start = Instant::now();
        let terminated = ProcessCleanup::terminate_group_gracefully(pid, 5_000).await;
        let elapsed = start.elapsed();

        let _ = child.wait();
        assert!(terminated);
        assert!(
            elapsed < Duration::from_millis(2_000),
            "expected early return once the child exited, waited {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn sigkills_child_that_ignores_sigterm() {
        let mut child =
            spawn_in_own_group("sh", &["-c", "trap '' TERM; while :; do sleep 0.2; done"]);
        let pid = child.id();

        // Let the shell install its SIGTERM trap before signalling, else it
        // dies on the default disposition and never reaches the SIGKILL path.
        tokio::time::sleep(Duration::from_millis(300)).await;

        let start = Instant::now();
        let terminated = ProcessCleanup::terminate_group_gracefully(pid, 400).await;
        let elapsed = start.elapsed();

        let _ = child.wait();
        assert!(terminated);
        assert!(
            elapsed >= Duration::from_millis(350),
            "expected to wait the grace deadline before SIGKILL, waited {elapsed:?}"
        );
    }
}

mod get_process_by_port_guards {
    use super::*;

    #[test]
    fn unbound_port_returns_none() {
        // Port 1 is in the privileged range and is effectively never bound by
        // this test process, so lsof finds no holder.
        assert!(ProcessCleanup::get_process_by_port(1).is_none());
    }

    #[test]
    fn high_unbound_port_returns_none() {
        assert!(ProcessCleanup::get_process_by_port(2).is_none());
    }
}

mod kill_by_pattern_safe_inputs {
    use super::*;

    #[test]
    fn pattern_with_slash_and_dot_is_accepted_but_matches_nothing() {
        // Safe characters (alnum, _, -, ., /) pass the guard; the pattern is
        // unique enough that pkill matches no process and returns 0.
        let unique = format!("var/run/systemprompt-no-match-{}.sock", std::process::id());
        assert_eq!(ProcessCleanup::kill_by_pattern(&unique), 0);
    }

    #[test]
    fn pattern_over_128_chars_is_rejected() {
        let too_long = "a".repeat(129);
        assert_eq!(ProcessCleanup::kill_by_pattern(&too_long), 0);
    }

    #[test]
    fn pattern_with_whitespace_is_rejected() {
        assert_eq!(ProcessCleanup::kill_by_pattern("foo bar"), 0);
    }
}

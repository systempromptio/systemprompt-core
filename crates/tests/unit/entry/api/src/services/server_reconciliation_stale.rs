//! `service_row_is_stale` — the decision that reaps a `services` row.
//!
//! Wrong in one direction it deletes the row of a live service and stops
//! tracking a running process; wrong in the other it adopts a PID the OS has
//! since handed to something else, and the next reap signals a stranger.
//!
//! The seam for this was already exported as `reconciliation_test_api` and had
//! no callers.

use systemprompt_api::services::server::reconciliation_test_api::service_row_is_stale;

const KEY: &str = "mcp_server";
const NAME: &str = "some-service";

// Why: `running` with no PID is a row that claims a process it cannot name.
// There is nothing to check liveness against, so it cannot be trusted.
#[test]
fn a_running_row_without_a_pid_is_stale() {
    assert!(service_row_is_stale("running", None, KEY, NAME));
}

// Why: PIDs are unsigned. A negative value is a corrupt row, not a process —
// it must be reaped rather than silently converted.
#[test]
fn a_running_row_with_a_negative_pid_is_stale() {
    assert!(service_row_is_stale("running", Some(-1), KEY, NAME));
}

#[test]
fn error_and_stopped_rows_are_always_stale() {
    for status in ["error", "stopped"] {
        assert!(
            service_row_is_stale(status, Some(1), KEY, NAME),
            "{status} is a terminal state and its row should be reaped"
        );
    }
}

// Why: a row mid-transition must be left alone. Reaping `starting` would
// delete the row of a service that is coming up, and the reap races the start
// every time a service is slow to bind.
#[test]
fn a_row_in_any_other_status_is_left_untouched() {
    for status in ["starting", "stopping", "pending", ""] {
        assert!(
            !service_row_is_stale(status, Some(1), KEY, NAME),
            "{status:?} is not a status this reap understands, so it must not act on it"
        );
    }
}

// Why: this is the recycled-PID case, and the reason the check is two-part
// rather than a bare liveness probe. The test process is unquestionably alive,
// so a `process_exists` check alone would call this row live and adopt it.
// It is not our subprocess, so it must still be stale — otherwise the next
// reap signals whatever now holds that PID.
#[test]
fn a_live_pid_that_is_not_our_subprocess_is_still_stale() {
    let live_but_unrelated = i32::try_from(std::process::id()).expect("pid fits in i32");

    assert!(
        service_row_is_stale("running", Some(live_but_unrelated), KEY, NAME),
        "a live PID that does not name our child must not be adopted"
    );
}

// Why: a PID that is not running at all is the ordinary case — the service
// died and left its row behind.
#[test]
fn a_running_row_whose_process_is_gone_is_stale() {
    let dead = free_pid();

    assert!(
        service_row_is_stale("running", Some(dead), KEY, NAME),
        "pid {dead} is not running, so its row is stale"
    );
}

/// A PID with no live process. Scans downward from the max so the search does
/// not collide with the low-numbered PIDs in active use, and asserts rather
/// than defaulting — a guessed-live PID would make the caller assert the
/// opposite of what it intends.
fn free_pid() -> i32 {
    for candidate in (30_000..40_000).rev() {
        if !std::path::Path::new(&format!("/proc/{candidate}")).exists() {
            return candidate;
        }
    }
    panic!("no free pid in the scanned range; the test cannot pick a dead process");
}

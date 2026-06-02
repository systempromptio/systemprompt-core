//! Environment-marker contract between the supervisor and the detached agent
//! and MCP children it spawns.
//!
//! The supervisor stamps these markers at spawn time; shutdown and
//! reconciliation read them back from `/proc/<pid>/environ` to confirm a
//! registry PID still names *this* installation's child before signalling it.
//! PIDs are recycled, and group-signalling a stale PID (`kill(-pid)`) could
//! reach an unrelated session leader — so a row is only ever signalled once
//! both the subprocess marker and the exact `name_key=service_name` pairing
//! are found.

pub const SUBPROCESS_MARKER_ENV: &str = "SYSTEMPROMPT_SUBPROCESS";
pub const AGENT_NAME_ENV: &str = "AGENT_NAME";
pub const MCP_SERVICE_ID_ENV: &str = "MCP_SERVICE_ID";

/// Convert an OS process id into the signed form `kill(2)` expects, rejecting
/// any value that would target more than that single process.
///
/// A `u32` above `i32::MAX` wraps to a negative `i32`, and `kill(2)` reads a
/// negative pid as a *process group* — `-1` broadcasts to **every** process the
/// caller may signal, and `0` means the caller's own group. Routing every pid
/// through this guard turns those cases into a no-op (`None`) instead of
/// letting a single-PID request escalate into a group or session-wide kill.
#[must_use]
pub fn signalable_pid(pid: u32) -> Option<i32> {
    if pid == 0 {
        return None;
    }
    i32::try_from(pid).ok()
}

#[must_use]
pub fn environ_identifies_child(environ: &[u8], name_key: &str, service_name: &str) -> bool {
    let marker = format!("{SUBPROCESS_MARKER_ENV}=1");
    let expected_name = format!("{name_key}={service_name}");

    let mut has_marker = false;
    let mut has_name = false;
    for entry in environ.split(|&b| b == 0) {
        if entry == marker.as_bytes() {
            has_marker = true;
        } else if entry == expected_name.as_bytes() {
            has_name = true;
        }
    }

    has_marker && has_name
}

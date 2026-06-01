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

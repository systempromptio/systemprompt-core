//! Identity contract for the detached agent and MCP children the supervisor
//! owns: the environment markers stamped at spawn time and the pure parsers
//! that read them back off a live process image.
//!
//! The supervisor stamps [`SUBPROCESS_MARKER_ENV`] and a `name_key=service`
//! pair into every child; shutdown, reconciliation, and port reclamation
//! confirm a registry PID still names *this* installation's child before
//! signalling it. PIDs are recycled, and group-signalling a stale PID
//! (`kill(-pid)`) could reach an unrelated session leader — so a row is only
//! ever signalled once both the marker and the exact pairing are found.
//!
//! Spawning and the platform-specific process probes live in
//! `systemprompt_loader::subprocess`; this module holds only data and pure
//! functions so the shared layer stays free of process I/O.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const SUBPROCESS_MARKER_ENV: &str = "SYSTEMPROMPT_SUBPROCESS";
pub const AGENT_NAME_ENV: &str = "AGENT_NAME";
pub const MCP_SERVICE_ID_ENV: &str = "MCP_SERVICE_ID";

pub const DEPLOYMENT_HOST_ENV: &str = "SYSTEMPROMPT_DEPLOYMENT_HOST";

// Why: Fly injects `FLY_APP_NAME` into deployed machines.
const FLY_HOST_ENV: &str = "FLY_APP_NAME";

pub fn deployment_host(lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    [DEPLOYMENT_HOST_ENV, FLY_HOST_ENV].iter().find_map(|name| {
        lookup(name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    })
}

pub fn is_deployment_host(lookup: impl Fn(&str) -> Option<String>) -> bool {
    deployment_host(lookup).is_some()
}

pub fn inherited_parent_env(lookup: impl Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = [
        DEPLOYMENT_HOST_ENV,
        FLY_HOST_ENV,
        "HOSTNAME",
        "PATH",
        "HOME",
    ]
    .iter()
    .filter_map(|name| lookup(name).map(|value| ((*name).to_owned(), value)))
    .collect();

    if let Some(entry) = crate::net::trusted_hosts_env_entry(&lookup) {
        env.push(entry);
    }

    env
}

#[must_use]
pub const fn identity_verification_supported() -> bool {
    cfg!(any(target_os = "linux", target_os = "macos"))
}

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

// Why: macOS `KERN_PROCARGS2` stores argc, exec path, NUL padding, argv, then
// environ. Skip argv by argc: argument strings can themselves look like
// environment entries.
#[must_use]
pub fn environ_from_procargs2(blob: &[u8]) -> Option<&[u8]> {
    const ARGC_LEN: usize = size_of::<i32>();

    let argc_bytes: [u8; ARGC_LEN] = blob.get(..ARGC_LEN)?.try_into().ok()?;
    let argc = usize::try_from(i32::from_ne_bytes(argc_bytes)).ok()?;

    let mut rest = blob.get(ARGC_LEN..)?;
    let exec_path_end = rest.iter().position(|&b| b == 0)?;
    rest = rest.get(exec_path_end + 1..)?;

    let argv_start = rest.iter().position(|&b| b != 0)?;
    rest = rest.get(argv_start..)?;

    for _ in 0..argc {
        let entry_end = rest.iter().position(|&b| b == 0)?;
        rest = rest.get(entry_end + 1..)?;
    }

    Some(rest)
}

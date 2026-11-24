//! Identity contract for the detached agent and MCP children the supervisor
//! owns: the environment markers stamped at spawn time and the pure parsers
//! that read them back off a live process image.
//!
//! The supervisor stamps [`SUBPROCESS_MARKER_ENV`], a `name_key=service`
//! pair and [`SPAWN_NONCE_ENV`] (the spawning process's per-boot nonce, also
//! recorded with the pid in the `services` row) into every child; shutdown,
//! reconciliation, and port reclamation confirm a registry PID still names
//! *this* installation's child of *that* boot before signalling it. A child
//! recorded before nonces existed (its row has a pid and no nonce) matches
//! only a process that carries the service marker and no nonce at all. PIDs are
//! recycled, and group-signalling a stale PID (`kill(-pid)`) could reach an
//! unrelated session leader — so a row is only ever signalled once both the
//! marker and the exact pairing are found.
//!
//! The API server is not a spawned child: `infra services serve` re-executes
//! itself once with [`API_SERVER_ENV`] so a later `stop api` can prove a port
//! holder is the API server before signalling it. It does not carry
//! [`SUBPROCESS_MARKER_ENV`], so no reconciliation pass mistakes it for a
//! supervised child.
//!
//! A foreground child of a CLI command (a `docker build`, a `cargo build`)
//! carries [`CLI_CHILD_ENV`] set to a token unique to that spawn, so an
//! interrupted command can prove a recorded pid is still that child before
//! stopping it. It too omits [`SUBPROCESS_MARKER_ENV`], so the supervisor
//! never adopts it.
//!
//! The markers carry identity only. What secrets a child receives is decided
//! by the grants in `grants`, never by a marker.
//!
//! Spawning and the platform-specific process probes live in
//! `systemprompt_loader::subprocess`; this module holds only data and pure
//! functions so the shared layer stays free of process I/O.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod grants;

pub use grants::{ProviderSecret, SecretGrant, SecretGrantError, is_api_held_env_name};
use systemprompt_identifiers::{ServiceName, SpawnNonce};

pub const SUBPROCESS_MARKER_ENV: &str = "SYSTEMPROMPT_SUBPROCESS";
pub const AGENT_NAME_ENV: &str = "AGENT_NAME";
pub const MCP_SERVICE_ID_ENV: &str = "MCP_SERVICE_ID";
pub const VERIFICATION_KEYS_ENV: &str = "VERIFICATION_KEYS";
pub const API_SERVER_ENV: &str = "SYSTEMPROMPT_API_SERVER";
pub const API_SERVER_SERVICE: &str = "api";
pub const CLI_CHILD_ENV: &str = "SYSTEMPROMPT_CLI_CHILD";
pub const SPAWN_NONCE_ENV: &str = "SYSTEMPROMPT_SPAWN_NONCE";

pub const DEPLOYMENT_HOST_ENV: &str = "SYSTEMPROMPT_DEPLOYMENT_HOST";

// Why: Fly injects `FLY_APP_NAME` into deployed machines.
pub const FLY_APP_NAME_ENV: &str = "FLY_APP_NAME";

pub const ANY_DEPLOYMENT_HOST_MARKERS: &[&str] = &[DEPLOYMENT_HOST_ENV, FLY_APP_NAME_ENV];

pub fn deployment_host(
    markers: &[&str],
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    markers.iter().find_map(|name| {
        lookup(name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    })
}

pub fn is_deployment_host(markers: &[&str], lookup: impl Fn(&str) -> Option<String>) -> bool {
    deployment_host(markers, lookup).is_some()
}

pub fn inherited_parent_env(lookup: impl Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = [
        DEPLOYMENT_HOST_ENV,
        FLY_APP_NAME_ENV,
        "HOSTNAME",
        "POD_NAME",
        "FLY_MACHINE_ID",
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
pub fn environ_identifies_child(
    environ: &[u8],
    name_key: &str,
    service_name: &ServiceName,
    nonce: Option<&SpawnNonce>,
) -> bool {
    let marker = format!("{SUBPROCESS_MARKER_ENV}=1");
    let expected_name = format!("{name_key}={service_name}");
    let nonce_prefix = format!("{SPAWN_NONCE_ENV}=");
    let expected_nonce = nonce.map(|nonce| format!("{nonce_prefix}{nonce}"));

    let mut has_marker = false;
    let mut has_name = false;
    let mut nonce_seen = false;
    let mut nonce_matches = false;
    for entry in environ.split(|&b| b == 0) {
        if entry == marker.as_bytes() {
            has_marker = true;
        } else if entry == expected_name.as_bytes() {
            has_name = true;
        } else if entry.starts_with(nonce_prefix.as_bytes()) {
            nonce_seen = true;
            nonce_matches = expected_nonce
                .as_ref()
                .is_some_and(|expected| entry == expected.as_bytes());
        }
    }

    let stamp_matches = match nonce {
        Some(_) => nonce_matches,
        None => !nonce_seen,
    };
    has_marker && has_name && stamp_matches
}

#[must_use]
pub fn environ_identifies_api_server(environ: &[u8], service_name: &ServiceName) -> bool {
    environ_has_entry(environ, API_SERVER_ENV, service_name)
}

#[must_use]
pub fn environ_identifies_cli_child(environ: &[u8], token: &ServiceName) -> bool {
    environ_has_entry(environ, CLI_CHILD_ENV, token)
}

fn environ_has_entry(environ: &[u8], key: &str, value: &ServiceName) -> bool {
    let expected = format!("{key}={value}");
    environ
        .split(|&b| b == 0)
        .any(|entry| entry == expected.as_bytes())
}

#[must_use]
pub fn environ_has_entries(environ: &[u8]) -> bool {
    environ
        .split(|&b| b == 0)
        .any(|entry| entry.contains(&b'='))
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

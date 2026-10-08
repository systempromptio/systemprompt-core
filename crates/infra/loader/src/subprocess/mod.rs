//! Process supervision for the agent and MCP children this installation owns,
//! and for its API server: the one place that spawns, identifies, probes and
//! stops them.
//!
//! # Spawning
//!
//! [`spawn_supervised`] is the only sanctioned way to start a child, and
//! [`mark_child`] stamps it with the environment markers that later prove it
//! is ours. Every spawn runs on one dedicated thread, which on Linux also arms
//! the parent-death signal.
//!
//! # Control
//!
//! [`is_running`], [`owns`], [`pids_listening_on`], [`terminate_gracefully`],
//! [`terminate_group_gracefully`] and [`stop_owned`] are async and never block
//! a runtime worker. A stop signals only a pid that is provably ours: the pid
//! the registry recorded *and* a matching [`ChildKind`] marker read back from
//! the live process. Outcomes are typed ([`Termination`], [`StopOutcome`]);
//! failures are [`SupervisionError`].
//!
//! # Identity
//!
//! The environment markers and the pure parsers that read them back live in
//! [`systemprompt_models::subprocess`]; the platform probes here
//! ([`live_pid_is_subprocess`], [`is_zombie`]) execute them against `/proc`
//! or `sysctl`. They are blocking primitives; async callers use [`owns`] and
//! [`is_running`].
//!
//! # Platform support
//!
//! The two halves of supervision have different reach, and conflating them is
//! what stranded ports on macOS:
//!
//! - **Identity and reap checks** ([`live_pid_is_subprocess`], [`is_zombie`])
//!   work on Linux, via `/proc`, and on macOS, via `sysctl(KERN_PROCARGS2)` and
//!   `proc_pidinfo`. Report the platform's coverage with
//!   [`identity_verification_supported`](systemprompt_models::subprocess::identity_verification_supported);
//!   where it is absent the checks are
//!   fail-closed stubs that never confirm an identity, so no process is ever
//!   signalled on a guess.
//! - **Parent-death prevention** is `prctl(PR_SET_PDEATHSIG)` and therefore
//!   Linux-only. macOS has no equivalent that survives `execve`, and the kqueue
//!   and pipe-EOF alternatives all require cooperation from the child binary —
//!   which is an arbitrary MCP server or agent executable here. A `SIGKILL`ed
//!   supervisor on macOS therefore leaves its children reparented to `launchd`
//!   and still holding their ports; the identity check above is what lets the
//!   next start reclaim them instead of erroring out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ServiceName;

mod control;
mod error;
mod ports;
mod spawn;

#[cfg(unix)]
mod posix;
#[cfg(windows)]
mod winnt;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::live_environ;
#[cfg(target_os = "linux")]
pub use linux::{is_zombie, live_pid_is_subprocess};

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
use darwin::live_environ;
#[cfg(target_os = "macos")]
pub use darwin::{is_zombie, live_pid_is_subprocess};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use unsupported::live_environ;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use unsupported::{is_zombie, live_pid_is_subprocess};

pub use control::{
    StopOutcome, Termination, is_running, owns, pids_listening_on, process_group, stop_owned,
    terminate_gracefully, terminate_group_gracefully,
};
pub use error::SupervisionError;
pub use ports::{parse_lsof_pids, parse_netstat_listeners};
pub use spawn::{
    ApiServerStamp, mark_child, place_in_own_process_group, spawn_owned_supervised,
    spawn_supervised, stamp_api_server,
};

/// Which kind of supervised process a pid is claimed to be; selects the
/// marker variable that names it. `Api` is the API server, stamped by
/// [`stamp_api_server`] rather than spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChildKind {
    Agent,
    Mcp,
    Api,
}

impl ChildKind {
    #[must_use]
    pub const fn marker_env(self) -> &'static str {
        match self {
            Self::Agent => systemprompt_models::subprocess::AGENT_NAME_ENV,
            Self::Mcp => systemprompt_models::subprocess::MCP_SERVICE_ID_ENV,
            Self::Api => systemprompt_models::subprocess::API_SERVER_ENV,
        }
    }

    #[must_use]
    pub fn identifies(self, environ: &[u8], service: &ServiceName) -> bool {
        match self {
            Self::Agent | Self::Mcp => systemprompt_models::subprocess::environ_identifies_child(
                environ,
                self.marker_env(),
                service,
            ),
            Self::Api => {
                systemprompt_models::subprocess::environ_identifies_api_server(environ, service)
            },
        }
    }
}

#[must_use]
pub fn api_server_service() -> ServiceName {
    ServiceName::new(systemprompt_models::subprocess::API_SERVER_SERVICE)
}

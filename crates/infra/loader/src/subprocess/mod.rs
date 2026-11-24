//! Process supervision for the agent and MCP children this installation owns,
//! and for its API server: the one place that spawns, identifies, probes and
//! stops them.
//!
//! # Spawning
//!
//! [`Spawner::spawn_supervised`] is the only sanctioned way to start a child,
//! and [`Spawner::mark_child`] stamps it with the environment markers that
//! later prove it is ours. A [`Spawner`] owns the one dedicated thread every
//! spawn runs on, which on Linux also arms the parent-death signal; the
//! composition root creates it once and injects it.
//!
//! # Control
//!
//! [`is_running`], [`owns`], [`pids_listening_on`], [`terminate_gracefully`],
//! [`terminate_group_gracefully`] and [`stop_owned`] are async and never block
//! a runtime worker. A stop signals only a pid that is provably ours: the pid
//! the registry recorded *and* a matching [`ChildClaim`] read back from the
//! live process: the service marker and the boot nonce ([`SpawnStamp`]) the
//! registry recorded with that pid. Ownership is checked again immediately
//! before a forced kill, so a pid that changed owner during the grace period
//! is never killed. Outcomes are typed ([`Termination`], [`StopOutcome`]);
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

use systemprompt_identifiers::{ServiceName, SpawnNonce};

mod control;
mod error;
mod foreground;
mod ports;
mod spawn;

#[cfg(unix)]
mod posix;
#[cfg(windows)]
mod winnt;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::is_zombie;
#[cfg(target_os = "linux")]
use linux::live_environ;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::is_zombie;
#[cfg(target_os = "macos")]
use darwin::live_environ;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use unsupported::is_zombie;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use unsupported::live_environ;

pub use control::{
    StopOutcome, SubprocessLiveness, Termination, is_running, owns, pids_listening_on,
    process_group, stop_owned, terminate_gracefully, terminate_group_gracefully,
};
pub use error::SupervisionError;
pub use foreground::ForegroundChildren;
pub use ports::{parse_lsof_pids, parse_netstat_listeners};
pub use spawn::{ApiServerStamp, Spawner, place_in_own_process_group, stamp_api_server};

/// Which kind of supervised process a pid is claimed to be.
///
/// The kind selects the marker variable that names it. `Api` is the API
/// server, stamped by [`stamp_api_server`] rather than spawned; `Command` is a
/// CLI command's foreground child, tracked by [`ForegroundChildren`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChildKind {
    Agent,
    Mcp,
    Api,
    Command,
}

impl ChildKind {
    #[must_use]
    pub const fn marker_env(self) -> &'static str {
        match self {
            Self::Agent => systemprompt_models::subprocess::AGENT_NAME_ENV,
            Self::Mcp => systemprompt_models::subprocess::MCP_SERVICE_ID_ENV,
            Self::Api => systemprompt_models::subprocess::API_SERVER_ENV,
            Self::Command => systemprompt_models::subprocess::CLI_CHILD_ENV,
        }
    }
}

/// What a `services` row recorded about the boot that spawned its child.
///
/// `Boot` is the nonce of the supervising process that spawned it.
/// `PreNonce` is a row with a pid and no nonce, written by a release that did
/// not stamp nonces: it matches only a process carrying the service marker and
/// no nonce variable, and it ends when that child is next restarted (every pid
/// written since carries a nonce). `Unrecorded` is a row with neither and
/// matches no process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnStamp {
    Boot(SpawnNonce),
    PreNonce,
    Unrecorded,
}

impl SpawnStamp {
    #[must_use]
    pub fn recorded(pid: Option<i32>, nonce: Option<&SpawnNonce>) -> Self {
        match (nonce, pid) {
            (Some(nonce), _) => Self::Boot(nonce.clone()),
            (None, Some(_)) => Self::PreNonce,
            (None, None) => Self::Unrecorded,
        }
    }
}

/// The identity a live process must carry before it is signalled.
///
/// Agent and MCP claims carry the service and the recorded [`SpawnStamp`];
/// the API server and a CLI command's foreground child are not spawned by a
/// [`Spawner`] and are identified by their own markers alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildClaim {
    Agent {
        service: ServiceName,
        stamp: SpawnStamp,
    },
    Mcp {
        service: ServiceName,
        stamp: SpawnStamp,
    },
    ApiServer,
    Command {
        token: ServiceName,
    },
}

impl ChildClaim {
    #[must_use]
    pub fn spawned(kind: ChildKind, service: &ServiceName, stamp: SpawnStamp) -> Self {
        let service = service.clone();
        match kind {
            ChildKind::Agent => Self::Agent { service, stamp },
            ChildKind::Mcp => Self::Mcp { service, stamp },
            ChildKind::Api => Self::ApiServer,
            ChildKind::Command => Self::Command { token: service },
        }
    }

    #[must_use]
    pub fn service(&self) -> ServiceName {
        match self {
            Self::Agent { service, .. } | Self::Mcp { service, .. } => service.clone(),
            Self::ApiServer => api_server_service(),
            Self::Command { token } => token.clone(),
        }
    }

    #[must_use]
    pub fn identifies(&self, environ: &[u8]) -> bool {
        use systemprompt_models::subprocess as identity;
        let spawned = |kind: ChildKind, service: &ServiceName, stamp: &SpawnStamp| match stamp {
            SpawnStamp::Boot(nonce) => {
                identity::environ_identifies_child(environ, kind.marker_env(), service, Some(nonce))
            },
            SpawnStamp::PreNonce => {
                identity::environ_identifies_child(environ, kind.marker_env(), service, None)
            },
            SpawnStamp::Unrecorded => false,
        };
        match self {
            Self::Agent { service, stamp } => spawned(ChildKind::Agent, service, stamp),
            Self::Mcp { service, stamp } => spawned(ChildKind::Mcp, service, stamp),
            Self::ApiServer => {
                identity::environ_identifies_api_server(environ, &api_server_service())
            },
            Self::Command { token } => identity::environ_identifies_cli_child(environ, token),
        }
    }
}

#[must_use]
pub fn live_pid_is_subprocess(pid: u32, claim: &ChildClaim) -> bool {
    live_environ(pid).is_some_and(|environ| claim.identifies(&environ))
}

#[must_use]
pub fn api_server_service() -> ServiceName {
    ServiceName::new(systemprompt_models::subprocess::API_SERVER_SERVICE)
}

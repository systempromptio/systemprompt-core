//! The readiness decision for a freshly spawned agent, as a pure function of
//! what one probe observed.
//!
//! Three facts are gathered per probe:
//!
//! - **The connect.** A TCP connect to `127.0.0.1:<port>`. It works on every
//!   platform.
//! - **The port's listening pids**, from
//!   `systemprompt_loader::subprocess::pids_listening_on`. It shells out to
//!   `lsof -t -iTCP:<port> -sTCP:LISTEN` on Linux and macOS and to `netstat
//!   -ano -p TCP` on Windows. It is unavailable (`None`) when the tool is
//!   missing or fails, as in a container image without `lsof`. It reports no
//!   holder when the host hides other processes' sockets.
//! - **The child.** Whether the spawned pid is still alive
//!   (`subprocess::is_running`, every platform), and whether it carries this
//!   agent's spawn marker (`subprocess::owns` with `ChildKind::Agent`). The
//!   marker is read from the child's environment, which is readable for a
//!   same-user child on Linux (`/proc/<pid>/environ`) and macOS
//!   (`KERN_PROCARGS2`). On Windows and other targets it is not readable, so
//!   [`MarkerCheck::Unavailable`] is reported.
//!
//! [`startup_verdict`] combines these facts. Its guarantees per host:
//!
//! - **Listeners identified** (a non-empty `lsof`/`netstat` answer). The agent
//!   is ready only when the spawned pid is among the listeners. Any other
//!   listener is a [`StartupVerdict::Foreign`] refusal. A foreign process is
//!   never taken for the agent.
//! - **Listeners not identifiable** (tool missing or failing, or an empty
//!   answer although the connect succeeded). The agent is ready when the
//!   connect succeeded, the spawned child is alive, and its marker is
//!   [`MarkerCheck::Verified`] (Linux, macOS) or [`MarkerCheck::Unavailable`]
//!   (Windows). A healthy agent is therefore never failed for want of a lookup.
//!   On such a host, a foreign listener that holds the port while the child is
//!   alive cannot be told apart from the agent.
//!
//! A child whose marker is read and does not match ([`MarkerCheck::Absent`])
//! is never ready.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Whether the spawned child's agent spawn marker could be read and matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerCheck {
    Verified,
    Absent,
    Unavailable,
}

/// What one readiness probe observed of a freshly spawned agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupObservation {
    pub connected: bool,
    pub listeners: Option<Vec<u32>>,
    pub child_alive: bool,
    pub marker: MarkerCheck,
}

/// The outcome of one readiness probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupVerdict {
    Ready,
    NotYet,
    Foreign(u32),
}

#[must_use]
pub fn startup_verdict(pid: u32, observed: &StartupObservation) -> StartupVerdict {
    if !observed.connected {
        return StartupVerdict::NotYet;
    }
    let child_is_ours = observed.child_alive && observed.marker != MarkerCheck::Absent;
    match observed.listeners.as_deref() {
        Some(listeners) if listeners.contains(&pid) => {
            if child_is_ours {
                StartupVerdict::Ready
            } else {
                StartupVerdict::NotYet
            }
        },
        Some(listeners) if !listeners.is_empty() => listeners
            .iter()
            .copied()
            .find(|holder| *holder != pid)
            .map_or(StartupVerdict::NotYet, StartupVerdict::Foreign),
        _ if child_is_ours => StartupVerdict::Ready,
        _ => StartupVerdict::NotYet,
    }
}

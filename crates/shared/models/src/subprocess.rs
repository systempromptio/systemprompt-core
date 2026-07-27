//! Spawning, identifying, and reaping the detached agent and MCP children the
//! supervisor owns.
//!
//! # Spawning
//!
//! [`spawn_supervised`] is the only sanctioned way to start a child. It runs
//! every spawn on one dedicated thread and asks the kernel to `SIGTERM` the
//! child if this process dies, so a crash, panic, or `SIGKILL` of the
//! supervisor cannot strand an agent holding a port.
//!
//! # Identity
//!
//! The supervisor stamps environment markers at spawn time; shutdown and
//! reconciliation read them back from `/proc/<pid>/environ` to confirm a
//! registry PID still names *this* installation's child before signalling it.
//! PIDs are recycled, and group-signalling a stale PID (`kill(-pid)`) could
//! reach an unrelated session leader — so a row is only ever signalled once
//! both the subprocess marker and the exact `name_key=service_name` pairing
//! are found.
//!
//! # Platform support
//!
//! Child supervision is **Linux-only**, matching where the server actually
//! runs. Both the identity check ([`live_pid_is_subprocess`]) and the reap
//! check ([`is_zombie`]) read `/proc`, and the parent-death signal is
//! `prctl(PR_SET_PDEATHSIG)`. Elsewhere they degrade to fail-closed stubs that
//! never confirm an identity, so no child is ever signalled and orphans must be
//! cleared by hand. The code compiles and runs on other platforms; it does not
//! supervise on them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::Command;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

pub const SUBPROCESS_MARKER_ENV: &str = "SYSTEMPROMPT_SUBPROCESS";
pub const AGENT_NAME_ENV: &str = "AGENT_NAME";
pub const MCP_SERVICE_ID_ENV: &str = "MCP_SERVICE_ID";

type SpawnReply = Sender<std::io::Result<u32>>;

/// Spawns `cmd` as a detached child that the kernel terminates if this process
/// dies, and returns its PID.
///
/// Every child is spawned on one dedicated thread. That is load-bearing, not
/// tidiness: `PR_SET_PDEATHSIG` is delivered when the *thread* that forked
/// exits, not when the process does. Forking from a tokio worker would tie a
/// live agent's lifetime to whichever worker happened to poll the spawn, and a
/// worker exiting early would kill a healthy child. This thread is created once
/// and never joined, so the signal fires only on real process death.
///
/// The child's `Child` handle is dropped without waiting, so it is never
/// reaped here — see [`is_zombie`], which liveness probes must consult.
///
/// On non-Linux targets this is a plain spawn: there is no parent-death signal,
/// so a supervisor that dies without draining leaves the child running.
pub fn spawn_supervised(cmd: Command) -> std::io::Result<u32> {
    let sender = spawner()
        .as_ref()
        .map_err(|e| std::io::Error::other(e.clone()))?;

    let (reply_tx, reply_rx) = channel();
    sender
        .send((cmd, reply_tx))
        .map_err(|disconnected| std::io::Error::other(disconnected.to_string()))?;
    reply_rx
        .recv()
        .map_err(|disconnected| std::io::Error::other(disconnected.to_string()))?
}

/// The spawner thread, started on first use and never joined.
///
/// The error is stored rather than returned so the thread is attempted exactly
/// once: if it cannot start, every later spawn fails with the same reason
/// instead of retrying a thread the OS has already refused.
fn spawner() -> &'static Result<Sender<(Command, SpawnReply)>, String> {
    static SPAWNER: OnceLock<Result<Sender<(Command, SpawnReply)>, String>> = OnceLock::new();
    SPAWNER.get_or_init(|| {
        let (tx, rx) = channel::<(Command, SpawnReply)>();
        std::thread::Builder::new()
            .name("subprocess-spawner".to_owned())
            .spawn(move || {
                while let Ok((mut cmd, reply)) = rx.recv() {
                    let outcome = spawn_on_this_thread(&mut cmd);
                    if reply.send(outcome).is_err() {
                        tracing::warn!(
                            "Spawn requester vanished before collecting the child pid; the child \
                             is unregistered and will only be cleaned up by its parent-death signal"
                        );
                    }
                }
            })
            .map(|_handle| tx)
            .map_err(|e| format!("could not start the subprocess spawner thread: {e}"))
    })
}

fn spawn_on_this_thread(cmd: &mut Command) -> std::io::Result<u32> {
    #[cfg(target_os = "linux")]
    arm_parent_death_signal(cmd);

    let child = cmd.spawn()?;
    let pid = child.id();
    #[expect(
        clippy::mem_forget,
        reason = "detached child: skip Child's drop-time wait so it keeps running after this \
                  returns; reaping is the caller's business via is_zombie"
    )]
    std::mem::forget(child);
    Ok(pid)
}

/// Asks the kernel to `SIGTERM` the child when this thread dies, closing the
/// window in which a `SIGKILL`ed or panicking supervisor strands its children.
///
/// This is the only `unsafe` in the crate graph. It buys an unconditional
/// guarantee: the alternative — having each child arm its own death signal at
/// startup — is safe code but only protects children that cooperate, and MCP
/// server binaries come from extension crates this repo does not own.
#[cfg(target_os = "linux")]
#[expect(
    unsafe_code,
    reason = "std::os::unix::process::CommandExt::pre_exec is an unsafe fn; there is no safe way \
              to run code in the forked child before exec, and the parent-death signal must be \
              armed there to cover children that never opt in"
)]
fn arm_parent_death_signal(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;

    let supervisor = std::process::id();

    // SAFETY: the closure runs in the forked child between `fork` and `execve`,
    // where only async-signal-safe calls are permitted. `prctl`, `getppid`, and
    // `_exit` are all on that list; nothing here allocates, locks, or logs.
    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // Why: if the supervisor died between `fork` and the `prctl` above,
            // the death signal has already been missed and this child would
            // outlive it forever. `getppid` no longer matching means exactly
            // that — the child has been reparented — so leave immediately.
            if libc::getppid() != supervisor as libc::pid_t {
                libc::_exit(0);
            }
            Ok(())
        });
    }
}

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

/// Confirm a *live* PID still names this installation's child by reading its
/// `/proc/<pid>/environ` and matching the spawn markers.
///
/// Fail-closed: an unreadable environ — or any non-Linux target, where
/// `/proc` does not exist — yields `false`, so an unverified PID is never
/// signalled. Callers must use this before any `kill`/`kill(-pid)` on a PID
/// loaded from the persisted service registry, because those PIDs outlive the
/// processes that minted them and are recycled by the kernel.
#[cfg(target_os = "linux")]
#[must_use]
pub fn live_pid_is_subprocess(pid: u32, name_key: &str, service_name: &str) -> bool {
    match std::fs::read(format!("/proc/{pid}/environ")) {
        Ok(environ) => environ_identifies_child(&environ, name_key, service_name),
        Err(e) => {
            tracing::warn!(pid, error = %e, "Could not read process environ to verify child identity");
            false
        },
    }
}

#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn live_pid_is_subprocess(pid: u32, _name_key: &str, service_name: &str) -> bool {
    tracing::warn!(
        pid,
        service = %service_name,
        "Child identity cannot be verified on this platform (no /proc), so this process will \
         not be signalled; supervision is Linux-only and the child must be stopped by hand"
    );
    false
}

/// Reports whether `pid` is a zombie — terminated but not yet reaped.
///
/// The supervisor never reaps the children it spawns (their `Child` handle is
/// forgotten), so a terminated child still answers `kill(pid, 0)`; liveness and
/// shutdown probes must consult this to avoid treating a dead child as alive.
/// Non-Linux targets have no `/proc` and always return `false`.
#[cfg(target_os = "linux")]
#[must_use]
pub fn is_zombie(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    // Why: The comm field is parenthesised and may contain spaces or `)`, so the
    // state char is the first token after the final `)`.
    let Some((_, after_comm)) = stat.rsplit_once(')') else {
        return false;
    };
    after_comm.split_whitespace().next() == Some("Z")
}

#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn is_zombie(_pid: u32) -> bool {
    false
}

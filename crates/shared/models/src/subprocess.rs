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

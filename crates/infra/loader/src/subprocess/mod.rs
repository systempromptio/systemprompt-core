//! Spawning and reaping the detached agent and MCP children the supervisor
//! owns.
//!
//! # Spawning
//!
//! [`spawn_supervised`] is the only sanctioned way to start a child. It runs
//! every spawn on one dedicated thread and, where the platform offers it, asks
//! the kernel to `SIGTERM` the child if this process dies, so a crash, panic,
//! or `SIGKILL` of the supervisor cannot strand an agent holding a port. The
//! spawner thread is started lazily; a failure to start it is returned to the
//! caller and retried on the next spawn rather than cached for the life of
//! the process.
//!
//! # Identity
//!
//! The environment markers and the pure parsers that read them back live in
//! [`systemprompt_models::subprocess`]; the platform probes here
//! ([`live_pid_is_subprocess`], [`is_zombie`]) are what execute them against
//! `/proc` or `sysctl`.
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


use std::process::Command;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, PoisonError};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{is_zombie, live_pid_is_subprocess};

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::{is_zombie, live_pid_is_subprocess};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use unsupported::{is_zombie, live_pid_is_subprocess};

type SpawnReply = Sender<std::io::Result<std::process::Child>>;
type SpawnRequest = (Command, SpawnReply);

static SPAWNER: Mutex<Option<Sender<SpawnRequest>>> = Mutex::new(None);

pub fn spawn_supervised(cmd: Command) -> std::io::Result<u32> {
    let child = spawn_owned_supervised(cmd)?;
    let pid = child.id();
    drop(child);
    Ok(pid)
}

pub fn spawn_owned_supervised(cmd: Command) -> std::io::Result<std::process::Child> {
    let sender = spawner()?;
    let (reply_tx, reply_rx) = channel();
    sender
        .send((cmd, reply_tx))
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    reply_rx
        .recv()
        .map_err(|error| std::io::Error::other(error.to_string()))?
}

fn spawner() -> std::io::Result<Sender<SpawnRequest>> {
    let mut slot = SPAWNER.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(sender) = slot.as_ref() {
        return Ok(sender.clone());
    }
    let sender = start_spawner_thread()?;
    Ok(slot.insert(sender).clone())
}

fn start_spawner_thread() -> std::io::Result<Sender<SpawnRequest>> {
    let (tx, rx) = channel::<SpawnRequest>();
    std::thread::Builder::new()
        .name("subprocess-spawner".to_owned())
        .spawn(move || {
            while let Ok((mut cmd, reply)) = rx.recv() {
                let outcome = spawn_on_this_thread(&mut cmd);
                if let Err(undelivered) = reply.send(outcome)
                    && let Ok(mut child) = undelivered.0
                {
                    if let Err(error) = child.kill() {
                        tracing::warn!(error = %error, "Failed to stop unclaimed subprocess");
                    }
                    if let Err(error) = child.wait() {
                        tracing::warn!(error = %error, "Failed to reap unclaimed subprocess");
                    }
                }
            }
        })
        .map(|_handle| tx)
}

fn spawn_on_this_thread(cmd: &mut Command) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "linux")]
    linux::arm_parent_death_signal(cmd);
    cmd.spawn()
}

// Why: On Unix, process group 0 assigns the child's PID as its process group
// ID.
#[cfg(unix)]
pub fn place_in_own_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
pub fn place_in_own_process_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

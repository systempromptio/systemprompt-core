//! Spawning, identifying, and reaping the detached agent and MCP children the
//! supervisor owns.
//!
//! # Spawning
//!
//! [`spawn_supervised`] is the only sanctioned way to start a child. It runs
//! every spawn on one dedicated thread and, where the platform offers it, asks
//! the kernel to `SIGTERM` the child if this process dies, so a crash, panic,
//! or `SIGKILL` of the supervisor cannot strand an agent holding a port.
//!
//! # Identity
//!
//! The supervisor stamps environment markers at spawn time; shutdown,
//! reconciliation, and port reclamation read them back off the live process to
//! confirm a registry PID still names *this* installation's child before
//! signalling it. PIDs are recycled, and group-signalling a stale PID
//! (`kill(-pid)`) could reach an unrelated session leader — so a row is only
//! ever signalled once both the subprocess marker and the exact
//! `name_key=service_name` pairing are found.
//!
//! # Platform support
//!
//! The two halves of supervision have different reach, and conflating them is
//! what stranded ports on macOS:
//!
//! - **Identity and reap checks** ([`live_pid_is_subprocess`], [`is_zombie`])
//!   work on Linux, via `/proc`, and on macOS, via `sysctl(KERN_PROCARGS2)` and
//!   `proc_pidinfo`. Report the platform's coverage with
//!   [`identity_verification_supported`]; where it is absent the checks are
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
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

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
    linux::arm_parent_death_signal(cmd);

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

// Why: pgid 0 makes the child its own group leader (pgid == pid), so the
// supervisor can signal the whole group on shutdown and reach any helper
// processes the child spawns, not just the child itself.
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

// Why: a `KERN_PROCARGS2` blob is `argc`, the exec path, NUL padding, `argc`
// argv entries, then the environment — all NUL-delimited in one buffer. The
// argv entries have to be skipped by count rather than searched past: an entry
// is matched whole by `environ_identifies_child`, so a command line such as
// `env MCP_SERVICE_ID=files …` would otherwise read as a marked environment and
// get an unrelated process signalled. Kept here rather than in `darwin` so the
// parse is unit-testable on every platform.
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

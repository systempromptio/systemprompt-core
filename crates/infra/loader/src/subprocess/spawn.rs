//! The one sanctioned way to start an agent or MCP child.
//!
//! Every spawn runs on one dedicated thread and, where the platform offers
//! it, the kernel is asked to `SIGTERM` the child if this process dies. The
//! parent-death signal follows the thread that forked the child, which is why
//! the spawner is a single long-lived thread rather than whichever runtime
//! worker happened to call: a worker that exits would take its children with
//! it. The thread is started lazily; a failure to start it is returned to the
//! caller and retried on the next spawn rather than cached.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::{Child, Command};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, PoisonError};

use systemprompt_identifiers::ServiceName;

use super::ChildKind;

type SpawnReply = Sender<std::io::Result<Child>>;
type SpawnRequest = (Command, SpawnReply);

static SPAWNER: Mutex<Option<Sender<SpawnRequest>>> = Mutex::new(None);

pub fn mark_child(cmd: &mut Command, kind: ChildKind, service: &ServiceName) {
    cmd.env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1")
        .env(kind.marker_env(), service.as_str());
}

pub fn spawn_supervised(cmd: Command) -> std::io::Result<u32> {
    let child = spawn_owned_supervised(cmd)?;
    let pid = child.id();
    drop(child);
    Ok(pid)
}

pub fn spawn_owned_supervised(cmd: Command) -> std::io::Result<Child> {
    let sender = spawner()?;
    let (reply_tx, reply_rx) = channel();
    sender
        .send((cmd, reply_tx))
        .map_err(std::io::Error::other)?;
    reply_rx.recv().map_err(std::io::Error::other)?
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

fn spawn_on_this_thread(cmd: &mut Command) -> std::io::Result<Child> {
    #[cfg(target_os = "linux")]
    super::linux::arm_parent_death_signal(cmd);
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

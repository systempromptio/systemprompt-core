//! The one sanctioned way to start an agent or MCP child, and the in-place
//! re-exec that stamps the API server with its identity marker.
//!
//! Every spawn runs on one dedicated thread and, where the platform offers
//! it, the kernel is asked to `SIGTERM` the child if this process dies. The
//! parent-death signal follows the thread that forked the child, which is why
//! the spawner is a single long-lived thread rather than whichever runtime
//! worker happened to call: a worker that exits would take its children with
//! it. [`Spawner`] owns that thread: it is started once, at the composition
//! root, and joined when the last handle to the spawner is dropped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::{Child, Command};
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use systemprompt_identifiers::ServiceName;

use super::ChildKind;

type SpawnReply = Sender<std::io::Result<Child>>;
type SpawnRequest = (Command, SpawnReply);

#[derive(Debug)]
pub struct Spawner {
    requests: Option<Sender<SpawnRequest>>,
    thread: Option<JoinHandle<()>>,
}

/// How [`stamp_api_server`] returned instead of re-executing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiServerStamp {
    Stamped,
    Unverifiable,
}

pub fn mark_child(cmd: &mut Command, kind: ChildKind, service: &ServiceName) {
    match kind {
        ChildKind::Agent | ChildKind::Mcp => {
            cmd.env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1");
        },
        ChildKind::Api => {},
    }
    cmd.env(kind.marker_env(), service.as_str());
}

// Why: the API server is launched by an operator, a service manager or a
// container runtime, never spawned by us, so the marker that later proves its
// identity cannot be stamped at spawn. `execve` keeps the pid, so replacing
// the image once with the marker added is invisible to whatever supervises
// the process. An environment that cannot be read back (no platform probe, a
// hardened binary) is left unstamped rather than re-executed in a loop.
#[cfg(unix)]
pub fn stamp_api_server() -> std::io::Result<ApiServerStamp> {
    use std::os::unix::process::CommandExt;

    let service = super::api_server_service();
    let Some(environ) = super::live_environ(std::process::id())
        .filter(|environ| systemprompt_models::subprocess::environ_has_entries(environ))
    else {
        return Ok(ApiServerStamp::Unverifiable);
    };
    if ChildKind::Api.identifies(&environ, &service) {
        return Ok(ApiServerStamp::Stamped);
    }
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.args(std::env::args_os().skip(1));
    mark_child(&mut cmd, ChildKind::Api, &service);
    Err(cmd.exec())
}

#[cfg(not(unix))]
pub const fn stamp_api_server() -> std::io::Result<ApiServerStamp> {
    Ok(ApiServerStamp::Unverifiable)
}

impl Spawner {
    pub fn new() -> std::io::Result<Self> {
        let (tx, rx) = channel::<SpawnRequest>();
        let thread = std::thread::Builder::new()
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
            })?;
        Ok(Self {
            requests: Some(tx),
            thread: Some(thread),
        })
    }

    pub fn spawn_supervised(&self, cmd: Command) -> std::io::Result<u32> {
        let child = self.spawn_owned_supervised(cmd)?;
        let pid = child.id();
        drop(child);
        Ok(pid)
    }

    pub fn spawn_owned_supervised(&self, cmd: Command) -> std::io::Result<Child> {
        let sender = self
            .requests
            .as_ref()
            .ok_or_else(|| std::io::Error::other("subprocess spawner is shut down"))?;
        let (reply_tx, reply_rx) = channel();
        sender
            .send((cmd, reply_tx))
            .map_err(std::io::Error::other)?;
        reply_rx.recv().map_err(std::io::Error::other)?
    }
}

impl Drop for Spawner {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("Subprocess spawner thread panicked");
        }
    }
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

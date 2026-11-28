//! Linux: bash, then sh; output that is not UTF-8 is read in the locale's
//! charset; a command runs in a process group of its own and carries the
//! parent-death signal, so it does not outlive the engine.
//!
//! The parent-death signal follows the thread that forked the child, not the
//! process. Every command is therefore forked on one thread this module owns:
//! a runtime worker that retired would otherwise take its commands with it.
//! The thread starts with the first command and is joined when its owner is
//! dropped, which is also when the commands it started are told to end.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::io;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use encoding_rs::{Encoding, UTF_8};
use parking_lot::Mutex;
use tokio::process::{Child, Command};
use tokio::runtime::Handle;

use super::Platform;
use super::posix::{self, Group, GroupError};
use crate::tools::shell::select::{ShellKind, ShellSpec};

const SHELLS: [&str; 2] = ["bash", "sh"];
const LOCALE_VARIABLES: [&str; 3] = ["LC_ALL", "LC_CTYPE", "LANG"];

#[derive(Debug, Default)]
pub(super) struct Linux {
    forker: Mutex<Option<Forker>>,
}

impl Platform for Linux {
    type Tree = Group;
    type TreeError = GroupError;

    const UNNAMED_SHELL: ShellKind = ShellKind::Posix;

    fn default_shell() -> ShellSpec {
        posix::first_shell(&SHELLS)
    }

    fn prepare(command: &mut Command, kind: ShellKind, script: &str) {
        posix::prepare(command, kind, script);
        arm_parent_death_signal(command);
    }

    fn spawn(&self, command: Command) -> io::Result<Child> {
        let runtime = Handle::try_current().map_err(io::Error::other)?;
        let mut slot = self.forker.lock();
        let forker = match slot.take() {
            Some(forker) => forker,
            None => Forker::start()?,
        };
        let spawned = forker.fork(command, runtime);
        *slot = Some(forker);
        spawned
    }

    fn adopt(child: &Child) -> Result<Group, GroupError> {
        Group::of(child)
    }

    fn decode(bytes: &[u8]) -> String {
        locale_charset()
            .and_then(|charset| charset.decode_without_bom_handling_and_without_replacement(bytes))
            .map_or_else(
                || String::from_utf8_lossy(bytes).into_owned(),
                Cow::into_owned,
            )
    }
}

fn locale_charset() -> Option<&'static Encoding> {
    let locale = LOCALE_VARIABLES
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))?;
    let (_, codeset) = locale.split_once('.')?;
    let codeset = codeset
        .split_once('@')
        .map_or(codeset, |(name, _modifier)| name);
    charset_named(codeset).filter(|charset| *charset != UTF_8)
}

fn charset_named(codeset: &str) -> Option<&'static Encoding> {
    Encoding::for_label(codeset.as_bytes()).or_else(|| {
        // Why: glibc spells the EUC charsets `eucJP` and `eucKR`, which the label
        // registry knows only as `euc-jp` and `euc-kr`.
        let region = codeset
            .get(..3)
            .filter(|prefix| prefix.eq_ignore_ascii_case("euc"))
            .and_then(|_| codeset.get(3..))?;
        Encoding::for_label(format!("euc-{region}").as_bytes())
    })
}

#[expect(
    unsafe_code,
    reason = "pre_exec is an unsafe fn, and the parent-death signal can only be set in the forked \
              child before it execs"
)]
fn arm_parent_death_signal(command: &mut Command) {
    let engine = match libc::pid_t::try_from(std::process::id()) {
        Ok(engine) => Some(engine),
        Err(error) => {
            tracing::warn!(error = %error, "the engine's process id is not a pid_t; the parent check is skipped");
            None
        },
    };
    // SAFETY: the closure runs in the forked child between `fork` and `execve`,
    // where only async-signal-safe calls are permitted. `prctl` and `getppid` are
    // on that list, and building an error from an errno neither allocates nor
    // locks.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(io::Error::last_os_error());
            }
            // Why: Linux does not deliver the parent-death signal when the parent died
            // before `PR_SET_PDEATHSIG` was set.
            if engine.is_some_and(|engine| libc::getppid() != engine) {
                return Err(io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}

struct Request {
    command: Command,
    runtime: Handle,
    reply: Sender<io::Result<Child>>,
}

#[derive(Debug)]
struct Forker {
    requests: Option<Sender<Request>>,
    thread: Option<JoinHandle<()>>,
}

impl Forker {
    fn start() -> io::Result<Self> {
        let (requests, queue) = channel::<Request>();
        let thread = std::thread::Builder::new()
            .name("shell-command-forker".to_owned())
            .spawn(move || {
                while let Ok(request) = queue.recv() {
                    let Request {
                        mut command,
                        runtime,
                        reply,
                    } = request;
                    let context = runtime.enter();
                    if reply.send(command.spawn()).is_err() {
                        tracing::debug!("shell command was started for a caller that had gone");
                    }
                    drop(context);
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            thread: Some(thread),
        })
    }

    fn fork(&self, command: Command, runtime: Handle) -> io::Result<Child> {
        let (reply, spawned) = channel();
        let request = Request {
            command,
            runtime,
            reply,
        };
        let accepted = self
            .requests
            .as_ref()
            .is_some_and(|requests| requests.send(request).is_ok());
        if !accepted {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        spawned.recv().map_err(io::Error::other)?
    }
}

impl Drop for Forker {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("shell command forker thread panicked");
        }
    }
}

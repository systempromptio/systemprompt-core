//! Builds the host's `tokio::process::Command` from a [`HostInvocation`].
//!
//! The child's environment is cleared and rebuilt from a short allowlist of
//! locale and lookup variables plus the invocation's own env, so no server
//! secret (database URL, signing keys, provider keys) reaches the host. The
//! child leads its own process group so the whole tree it starts can be
//! signalled at once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::Stdio;

use crate::host::HostInvocation;

pub const INHERITED_ENV: &[&str] = &["PATH", "LANG", "LC_ALL", "TERM", "TZ"];

#[must_use]
pub(super) fn build_command(invocation: &HostInvocation) -> tokio::process::Command {
    let mut command = std::process::Command::new(&invocation.program);
    command.args(&invocation.args).env_clear();
    for key in INHERITED_ENV {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .envs(invocation.env.iter().map(|(k, v)| (k, v)))
        .current_dir(&invocation.cwd)
        .stdin(if invocation.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    place_in_own_process_group(&mut command);
    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(true);
    command
}

#[cfg(unix)]
fn place_in_own_process_group(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
const fn place_in_own_process_group(_command: &mut std::process::Command) {}

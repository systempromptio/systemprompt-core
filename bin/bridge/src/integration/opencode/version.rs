//! The installed `OpenCode` CLI version, read from `opencode --version`.
//!
//! The bridge writes only the V2 config shape, which `OpenCode` below
//! [`MIN_CLIENT_VERSION`] does not read, so doctor reports an older client
//! as an error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub use systemprompt_models::bridge::opencode::MIN_CLIENT_VERSION;

use super::config;

const DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum ClientVersion {
    NotInstalled,
    Unreadable(ClientProbeError),
    Found(semver::Version),
}

impl ClientVersion {
    #[must_use]
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Found(version) if *version >= MIN_CLIENT_VERSION)
    }
}

#[must_use]
pub fn parse_version(output: &str) -> Option<semver::Version> {
    output
        .split_whitespace()
        .map(|token| token.trim_start_matches('v'))
        .find_map(|token| semver::Version::parse(token).ok())
}

#[must_use]
pub fn probe_client_version() -> ClientVersion {
    match run_client(&["--version"], DEADLINE) {
        ClientRun::NotInstalled => ClientVersion::NotInstalled,
        ClientRun::Failed(error) => ClientVersion::Unreadable(error),
        ClientRun::Output(output) => parse_version(&output).map_or_else(
            || ClientVersion::Unreadable(ClientProbeError::Unrecognised(output.trim().to_owned())),
            ClientVersion::Found,
        ),
    }
}

pub(super) enum ClientRun {
    NotInstalled,
    Failed(ClientProbeError),
    Output(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ClientProbeError {
    #[error("capture file: {0}")]
    Capture(#[source] std::io::Error),
    #[error("spawn: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("wait: {0}")]
    Wait(#[source] std::io::Error),
    #[error("read: {0}")]
    Read(#[source] std::io::Error),
    #[error("timed out")]
    TimedOut,
    #[error("exit {0}")]
    Exit(i32),
    #[error("unrecognised output {0:?}")]
    Unrecognised(String),
    #[error("`opencode debug config` is not JSON: {0}")]
    Config(#[source] serde_json::Error),
}

pub(super) fn run_client(args: &[&str], deadline: Duration) -> ClientRun {
    let Some(binary) =
        crate::integration::app_launch::cli_binary(config::BINARY, &config::extra_bin_dirs())
    else {
        return ClientRun::NotInstalled;
    };
    // Why: `opencode` may start a background service that inherits stdout and
    // outlives the command; a pipe would never reach EOF, a file does not wait.
    let sink = match tempfile::NamedTempFile::new() {
        Ok(file) => file,
        Err(error) => return ClientRun::Failed(ClientProbeError::Capture(error)),
    };
    let capture = match sink.reopen() {
        Ok(file) => file,
        Err(error) => return ClientRun::Failed(ClientProbeError::Capture(error)),
    };
    let mut command = Command::new(&binary);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(capture))
        .stderr(Stdio::null());
    #[cfg(windows)]
    crate::winproc::no_window(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return ClientRun::Failed(ClientProbeError::Spawn(error)),
    };
    let status = match wait_until(&mut child, deadline) {
        Ok(status) => status,
        Err(error) => return ClientRun::Failed(error),
    };
    let output = match std::fs::read_to_string(sink.path()) {
        Ok(output) => output,
        Err(error) => return ClientRun::Failed(ClientProbeError::Read(error)),
    };
    if !status.success() {
        return ClientRun::Failed(ClientProbeError::Exit(status.code().unwrap_or(-1)));
    }
    ClientRun::Output(output)
}

fn wait_until(
    child: &mut std::process::Child,
    deadline: Duration,
) -> Result<std::process::ExitStatus, ClientProbeError> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            },
            Ok(None) => {
                // Why: the child was spawned here, so it is provably ours to stop.
                if let Err(error) = child.kill() {
                    tracing::warn!(error = %error, "opencode did not stop");
                }
                if let Err(error) = child.wait() {
                    tracing::warn!(error = %error, "opencode was not reaped");
                }
                return Err(ClientProbeError::TimedOut);
            },
            Err(error) => return Err(ClientProbeError::Wait(error)),
        }
    }
}

//! The installed `OpenCode` CLI version, read from `opencode --version`.
//!
//! The bridge writes only the V2 config shape, which `OpenCode` below
//! [`MIN_CLIENT_VERSION`] does not read, so doctor reports an older client
//! as an error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub use systemprompt_models::bridge::opencode::MIN_CLIENT_VERSION;

use super::config;

const DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientVersion {
    NotInstalled,
    Unreadable(String),
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
    let Some(binary) =
        crate::integration::app_launch::cli_binary(config::BINARY, &config::extra_bin_dirs())
    else {
        return ClientVersion::NotInstalled;
    };
    let mut command = Command::new(&binary);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    crate::winproc::no_window(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return ClientVersion::Unreadable(format!("spawn: {error}")),
    };
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < DEADLINE => {
                std::thread::sleep(Duration::from_millis(50));
            },
            Ok(None) => {
                // Why: the child was spawned here, so it is provably ours to stop.
                if let Err(error) = child.kill() {
                    tracing::warn!(error = %error, "opencode --version did not stop");
                }
                if let Err(error) = child.wait() {
                    tracing::warn!(error = %error, "opencode --version was not reaped");
                }
                return ClientVersion::Unreadable("timed out".to_owned());
            },
            Err(error) => return ClientVersion::Unreadable(format!("wait: {error}")),
        }
    };
    let mut output = String::new();
    if let Some(mut stdout) = child.stdout.take()
        && let Err(error) = stdout.read_to_string(&mut output)
    {
        return ClientVersion::Unreadable(format!("read: {error}"));
    }
    if !status.success() {
        return ClientVersion::Unreadable(format!("exit {}", status.code().unwrap_or(-1)));
    }
    parse_version(&output).map_or_else(
        || ClientVersion::Unreadable(format!("unrecognised output {:?}", output.trim())),
        ClientVersion::Found,
    )
}

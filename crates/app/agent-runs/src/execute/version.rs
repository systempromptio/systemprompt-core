//! The pre-spawn client version check for hosts that declare a
//! [`VersionRequirement`].
//!
//! The probe runs the host program with the requirement's args, the run's
//! own environment and cwd, under [`PROBE_TIMEOUT`]; the first
//! `MAJOR.MINOR.PATCH` token in stdout or stderr is the client version.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use super::command::build_command;
use crate::host::{HostInvocation, VersionRequirement};

pub const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("{program} version probe failed")]
    Probe {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{program} version probe timed out")]
    TimedOut { program: String },
    #[error("{program} printed no version; requires >= {minimum}")]
    Unreadable {
        program: String,
        minimum: semver::Version,
    },
    #[error("{program} {found} is below the required {minimum}")]
    TooOld {
        program: String,
        found: semver::Version,
        minimum: semver::Version,
    },
}

pub(super) async fn check(
    requirement: &VersionRequirement,
    invocation: &HostInvocation,
) -> Result<(), VersionError> {
    let probe = HostInvocation {
        program: invocation.program.clone(),
        args: requirement.args.iter().map(|a| (*a).to_owned()).collect(),
        env: invocation.env.clone(),
        cwd: invocation.cwd.clone(),
        stdin: None,
    };
    let output = match tokio::time::timeout(PROBE_TIMEOUT, build_command(&probe).output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(source)) => {
            return Err(VersionError::Probe {
                program: probe.program,
                source,
            });
        },
        Err(_elapsed) => {
            return Err(VersionError::TimedOut {
                program: probe.program,
            });
        },
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let Some(found) = first_version(&text) else {
        return Err(VersionError::Unreadable {
            program: probe.program,
            minimum: requirement.minimum.clone(),
        });
    };
    if found < requirement.minimum {
        return Err(VersionError::TooOld {
            program: probe.program,
            found,
            minimum: requirement.minimum.clone(),
        });
    }
    Ok(())
}

#[must_use]
pub fn first_version(text: &str) -> Option<semver::Version> {
    text.split(|c: char| c.is_whitespace() || c == '/')
        .map(|token| token.trim_start_matches('v'))
        .find_map(|token| semver::Version::parse(token).ok())
}

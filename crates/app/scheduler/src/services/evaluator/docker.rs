//! Docker subprocesses retain transport settings without ambient registry
//! credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::process::Command;

pub(super) fn command(executable: &Path) -> std::io::Result<(Command, tempfile::TempDir)> {
    let configuration = tempfile::Builder::new()
        .prefix("evaluator-docker-")
        .tempdir()?;
    let mut command = Command::new(executable);
    command.env_clear();
    for name in ["PATH", "DOCKER_HOST", "SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .env("HOME", configuration.path())
        .env("USERPROFILE", configuration.path())
        .env("DOCKER_CONFIG", configuration.path());
    Ok((command, configuration))
}

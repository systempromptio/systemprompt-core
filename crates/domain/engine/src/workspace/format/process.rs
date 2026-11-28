//! Running a resolved formatter as a child process with a timeout.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::Stdio;
use std::time::Duration;

use super::{FormatError, ResolvedFormatter};
use crate::workspace::platform;

const STDERR_TAIL_CHARS: usize = 2000;

pub(super) async fn run(
    resolved: &ResolvedFormatter,
    timeout: Duration,
) -> Result<(), FormatError> {
    let mut command = platform::command(&resolved.program);
    command
        .args(&resolved.args)
        .current_dir(&resolved.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = command.spawn().map_err(|source| FormatError::Spawn {
        name: resolved.name.clone(),
        program: resolved.program.clone(),
        source,
    })?;
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(result) => result.map_err(|source| FormatError::Wait {
            name: resolved.name.clone(),
            source,
        })?,
        Err(_elapsed) => {
            tracing::warn!(formatter = %resolved.name, timeout = ?timeout, "formatter timed out");
            return Err(FormatError::Timeout {
                name: resolved.name.clone(),
                after: timeout,
            });
        },
    };
    if output.status.success() {
        return Ok(());
    }
    Err(FormatError::Failed {
        name: resolved.name.clone(),
        code: output.status.code(),
        stderr: tail(&String::from_utf8_lossy(&output.stderr), STDERR_TAIL_CHARS),
    })
}

fn tail(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.trim_end().to_owned();
    }
    text.chars()
        .skip(count - max_chars)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

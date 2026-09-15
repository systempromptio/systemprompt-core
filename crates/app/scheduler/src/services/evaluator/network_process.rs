//! Docker status calls, name/label validation and bounded child waits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{
    Child, ContainerExecution, ExitStatus, Instant, Path, SchedulerError, SchedulerResult, Stdio,
    docker_command,
};

pub(crate) fn docker_status(docker: &Path, arguments: &[&str]) -> SchedulerResult<()> {
    let (mut command, _docker_configuration) = docker_command(docker)?;
    let status = command
        .args(arguments)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(SchedulerError::config_error(format!(
            "Docker {} failed",
            arguments.first().copied().unwrap_or("command")
        )));
    }
    Ok(())
}

pub(crate) fn safe_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

pub(crate) fn safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

pub(crate) fn private_log(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

// Why: cancellation runs from `Drop`, which cannot be async, so the bounded
// wait has to block. On a multi-threaded runtime it is handed to
// `block_in_place` so the ten seconds are spent off the async scheduler rather
// than stalling a worker that still owns other tasks.
pub(crate) fn wait_bounded(child: &mut Child) -> std::io::Result<ExitStatus> {
    match tokio::runtime::Handle::try_current().map(|handle| handle.runtime_flavor()) {
        Ok(tokio::runtime::RuntimeFlavor::MultiThread) => {
            tokio::task::block_in_place(|| poll_until_exit(child))
        },
        _ => poll_until_exit(child),
    }
}

fn poll_until_exit(child: &mut Child) -> std::io::Result<ExitStatus> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if started.elapsed().as_secs() >= 10 {
            child.kill()?;
            child.wait()?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Container command timed out",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

impl Drop for ContainerExecution {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => {},
            Ok(None) => {
                if let Err(error) = self.cancel() {
                    tracing::error!(error = %error, container = %self.name, "Evaluator cleanup requires reconciliation");
                }
            },
            Err(error) => {
                tracing::error!(error = %error, container = %self.name, "Cannot establish evaluator child state");
            },
        }
    }
}

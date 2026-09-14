//! Per-execution Docker network isolation and process cleanup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    Child, Command, ContainerExecution, ExitStatus, Instant, Path, PathBuf, SchedulerError,
    SchedulerResult, Stdio,
};
#[derive(Debug)]
pub struct ExecutionNetwork {
    docker: PathBuf,
    name: String,
    relay: Option<String>,
    removed: bool,
    owner_label: String,
    execution_label: String,
}

impl ExecutionNetwork {
    pub fn create(
        docker: PathBuf,
        name: String,
        owner: &str,
        execution: &str,
    ) -> SchedulerResult<Self> {
        if !docker.is_absolute()
            || !safe_name(&name)
            || !safe_label(owner)
            || !safe_label(execution)
            || matches!(name.as_str(), "host" | "bridge" | "default" | "none")
        {
            return Err(SchedulerError::config_error(
                "Invalid evaluator network configuration",
            ));
        }
        docker_status(
            &docker,
            &[
                "network",
                "create",
                "--internal",
                "--label",
                "systemprompt.evaluator=true",
                "--label",
                &format!("systemprompt.evaluator.owner={owner}"),
                "--label",
                &format!("systemprompt.evaluator.execution={execution}"),
                &name,
            ],
        )?;
        let network = Self {
            docker,
            name,
            relay: None,
            removed: false,
            owner_label: owner.to_owned(),
            execution_label: execution.to_owned(),
        };
        network.verify(&[])?;
        Ok(network)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start_relay(
        &mut self,
        image: &str,
        control_network: &str,
        upstream: &str,
        name: String,
    ) -> SchedulerResult<()> {
        if !safe_name(control_network)
            || matches!(control_network, "host" | "bridge" | "default" | "none")
            || !safe_name(&name)
            || !image.contains("@sha256:")
        {
            return Err(SchedulerError::config_error(
                "Relay requires pinned image and dedicated control network",
            ));
        }
        docker_status(
            &self.docker,
            &[
                "run",
                "-d",
                "--name",
                &name,
                "--label",
                "systemprompt.evaluator=true",
                "--label",
                &format!("systemprompt.evaluator.owner={}", self.owner_label),
                "--label",
                &format!("systemprompt.evaluator.execution={}", self.execution_label),
                "--network",
                &self.name,
                "--read-only",
                "--cap-drop=ALL",
                "--security-opt=no-new-privileges",
                "--pids-limit=64",
                "--memory=256m",
                "--cpus=.25",
                "--user=1002:1002",
                "-e",
                &format!("SYSTEMPROMPT_RELAY_UPSTREAM={upstream}"),
                image,
            ],
        )?;
        docker_status(
            &self.docker,
            &["network", "connect", control_network, &name],
        )?;
        self.relay = Some(name.clone());
        self.verify(&[name])
    }

    pub fn verify(&self, expected: &[String]) -> SchedulerResult<()> {
        let output = Command::new(&self.docker)
            .args(["network", "inspect", &self.name])
            .output()?;
        if !output.status.success() {
            return Err(SchedulerError::config_error(
                "Execution network inspection failed",
            ));
        }
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| SchedulerError::Internal(error.to_string()))?;
        let network = value
            .as_array()
            .and_then(|values| values.first())
            .ok_or_else(|| {
                SchedulerError::config_error("Execution network inspection returned no network")
            })?;
        if network.get("Internal").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(SchedulerError::config_error(
                "Execution network is not internal",
            ));
        }
        let mut actual = network
            .get("Containers")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flat_map(|containers| containers.values())
            .filter_map(|container| container.get("Name").and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut expected = expected.to_vec();
        actual.sort();
        expected.sort();
        if actual != expected {
            return Err(SchedulerError::config_error(
                "Execution network has unexpected members",
            ));
        }
        Ok(())
    }

    pub fn cleanup(&mut self) -> SchedulerResult<()> {
        if let Some(relay) = self.relay.take() {
            docker_status(&self.docker, &["rm", "--force", &relay])?;
        }
        docker_status(&self.docker, &["network", "rm", &self.name])?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for ExecutionNetwork {
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        if let Some(relay) = self.relay.take()
            && let Err(error) = docker_status(&self.docker, &["rm", "--force", &relay])
        {
            tracing::error!(relay = %relay, %error, "evaluator relay container leaked; reconciliation must remove it");
        }
        if let Err(error) = docker_status(&self.docker, &["network", "rm", &self.name]) {
            tracing::error!(network = %self.name, %error, "evaluator network leaked; reconciliation must remove it");
        }
    }
}

pub(super) fn docker_status(docker: &Path, arguments: &[&str]) -> SchedulerResult<()> {
    let status = Command::new(docker)
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

pub(super) fn safe_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

pub(super) fn safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

pub(super) fn private_log(path: &Path) -> std::io::Result<std::fs::File> {
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
pub(super) fn wait_bounded(child: &mut Child) -> std::io::Result<ExitStatus> {
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

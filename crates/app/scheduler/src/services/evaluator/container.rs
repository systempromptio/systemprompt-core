//! Owned client containers with bounded output and explicit cancellation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::client::{ClientPurpose, NativeClient};
use super::docker::command as docker_command;
use crate::{SchedulerError, SchedulerResult};

#[path = "container_builder.rs"]
mod builder;
#[path = "network.rs"]
mod network;
#[path = "container_verification.rs"]
mod verification;
pub use builder::ContainerLaunchBuilder;
pub use network::ExecutionNetwork;
use network::{private_log, safe_label, safe_name, wait_bounded};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus, Stdio};
use std::time::Instant;
use systemprompt_loader::subprocess::{place_in_own_process_group, spawn_owned_supervised};
pub use verification::{ClientVerifier, PinnedClientVerifier};

#[derive(Debug)]
pub struct ContainerExecution {
    child: Child,
    _docker_configuration: tempfile::TempDir,
    docker: PathBuf,
    name: String,
    output: PathBuf,
    errors: PathBuf,
    started: Instant,
    timeout_seconds: u32,
    max_output_bytes: u64,
    max_writable_bytes: u64,
}

#[derive(Debug)]
pub struct ContainerLaunch {
    docker: PathBuf,
    image: String,
    network: String,
    name: String,
    directory: PathBuf,
    output_stem: String,
    owner_label: String,
    execution_label: String,
    lease: Option<systemprompt_evaluation::repository::experiments::ExecutionLease>,
    runtime_user: String,
    verifier: std::sync::Arc<dyn ClientVerifier>,
}

impl ContainerLaunch {
    pub fn builder(docker: PathBuf, directory: PathBuf) -> ContainerLaunchBuilder {
        ContainerLaunchBuilder::new(docker, directory)
    }

    pub fn start(
        &self,
        client: &NativeClient,
        prompt: &str,
    ) -> SchedulerResult<ContainerExecution> {
        self.start_for(client, ClientPurpose::Execution, prompt)
    }

    pub fn start_for(
        &self,
        client: &NativeClient,
        purpose: ClientPurpose,
        prompt: &str,
    ) -> SchedulerResult<ContainerExecution> {
        self.verifier.verify(self, client)?;
        let output = self
            .directory
            .join(format!("{}-events.jsonl", self.output_stem));
        let log = private_log(&output)?;
        let errors_path = self
            .directory
            .join(format!("{}-stderr.log", self.output_stem));
        let errors = private_log(&errors_path)?;
        let (mut command, docker_configuration) = docker_command(&self.docker)?;
        command.args([
            "run",
            "--rm",
            "--name",
            &self.name,
            "--label",
            "systemprompt.evaluator=true",
            "--label",
            &format!("systemprompt.evaluator.owner={}", self.owner_label),
            "--label",
            &format!("systemprompt.evaluator.execution={}", self.execution_label),
            "--network",
            &self.network,
            "--read-only",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--pids-limit=128",
            "--memory=2g",
            "--cpus=1",
            &format!("--user={}", self.runtime_user),
            "--tmpfs=/tmp:rw,nosuid,nodev,size=256m",
            "--workdir=/home/tester/work",
        ]);
        if let Some(lease) = &self.lease {
            command.args([
                "--label",
                &format!("systemprompt.evaluator.worker={}", lease.worker_id),
                "--label",
                &format!("systemprompt.evaluator.fence={}", lease.fencing_token),
            ]);
        }
        command.arg("--mount").arg(format!(
            "type=bind,src={},dst=/home/tester",
            self.directory.join("home").display()
        ));
        command
            .arg("--env-file")
            .arg(self.directory.join("client.env"));
        command.arg(&self.image).args(
            client
                .arguments_for(purpose, prompt)
                .map_err(|error| SchedulerError::config_error(error.to_string()))?,
        );
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(errors));
        place_in_own_process_group(&mut command);
        let child = spawn_owned_supervised(command)?;
        Ok(ContainerExecution {
            child,
            _docker_configuration: docker_configuration,
            docker: self.docker.clone(),
            name: self.name.clone(),
            output,
            errors: errors_path,
            started: Instant::now(),
            timeout_seconds: client.limits().active_timeout_seconds,
            max_output_bytes: client.limits().max_artifact_bytes,
            max_writable_bytes: 512 * 1024 * 1024,
        })
    }
}

impl ContainerExecution {
    pub fn output_paths(&self) -> (&Path, &Path) {
        (&self.output, &self.errors)
    }
    pub fn poll(&mut self) -> SchedulerResult<Option<ExitStatus>> {
        if self.started.elapsed().as_secs() > u64::from(self.timeout_seconds)
            || std::fs::metadata(&self.output)?
                .len()
                .saturating_add(std::fs::metadata(&self.errors)?.len())
                > self.max_output_bytes
            || directory_bytes(&self.directory()?)? > self.max_writable_bytes
        {
            self.cancel()?;
            return Err(SchedulerError::ConfigError {
                message: "Client exceeded execution time, output, or writable-storage limit"
                    .to_owned(),
            });
        }
        Ok(self.child.try_wait()?)
    }

    fn directory(&self) -> SchedulerResult<PathBuf> {
        Ok(self
            .output
            .parent()
            .ok_or_else(|| {
                SchedulerError::config_error("Evaluator output path has no workspace parent")
            })?
            .join("home"))
    }

    pub fn cancel(&mut self) -> SchedulerResult<()> {
        let (mut command, _docker_configuration) = docker_command(&self.docker)?;
        command
            .args(["rm", "--force", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut cleanup = spawn_owned_supervised(command)?;
        let status = wait_bounded(&mut cleanup)?;
        if !status.success() {
            return Err(SchedulerError::ConfigError {
                message: "Container cleanup was not acknowledged".to_owned(),
            });
        }
        wait_bounded(&mut self.child)?;
        Ok(())
    }
}

fn directory_bytes(root: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && directory == root => {
                return Ok(0);
            },
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let metadata = std::fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    Ok(total)
}

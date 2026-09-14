//! Owned client containers with bounded output and explicit cancellation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::client::{ClientPurpose, NativeClient};
use crate::{SchedulerError, SchedulerResult};

#[path = "network.rs"]
mod network;
pub use network::ExecutionNetwork;
use network::{private_log, safe_label, safe_name, wait_bounded};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Instant;
use systemprompt_models::subprocess::{place_in_own_process_group, spawn_owned_supervised};

#[derive(Debug)]
pub struct ContainerExecution {
    child: Child,
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
    runtime_user: String,
}

impl ContainerLaunch {
    pub fn builder(docker: PathBuf, directory: PathBuf) -> ContainerLaunchBuilder {
        ContainerLaunchBuilder {
            docker,
            directory,
            image: None,
            network: None,
            name: None,
            output_stem: "client".to_owned(),
            owner_label: String::new(),
            execution_label: String::new(),
        }
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
        let output = self
            .directory
            .join(format!("{}-events.jsonl", self.output_stem));
        let log = private_log(&output)?;
        let errors_path = self
            .directory
            .join(format!("{}-stderr.log", self.output_stem));
        let errors = private_log(&errors_path)?;
        let mut command = Command::new(&self.docker);
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
        command.arg("--mount").arg(format!(
            "type=bind,src={},dst=/home/tester",
            self.directory.join("home").display()
        ));
        command
            .arg("--env-file")
            .arg(self.directory.join("client.env"));
        command
            .arg(&self.image)
            .args(client.arguments_for(purpose, prompt));
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(errors));
        place_in_own_process_group(&mut command);
        let child = spawn_owned_supervised(command)?;
        Ok(ContainerExecution {
            child,
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
        let mut command = Command::new(&self.docker);
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

#[derive(Debug)]
pub struct ContainerLaunchBuilder {
    docker: PathBuf,
    directory: PathBuf,
    image: Option<String>,
    network: Option<String>,
    name: Option<String>,
    output_stem: String,
    owner_label: String,
    execution_label: String,
}

impl ContainerLaunchBuilder {
    pub fn image(mut self, image: String) -> Self {
        self.image = Some(image);
        self
    }
    pub fn network(mut self, network: String) -> Self {
        self.network = Some(network);
        self
    }
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    pub fn output_stem(mut self, output_stem: impl Into<String>) -> Self {
        self.output_stem = output_stem.into();
        self
    }
    pub fn ownership(mut self, owner: impl Into<String>, execution: impl Into<String>) -> Self {
        self.owner_label = owner.into();
        self.execution_label = execution.into();
        self
    }
    pub fn build(self) -> SchedulerResult<ContainerLaunch> {
        let invalid = || {
            SchedulerError::ConfigError { message: "Evaluator requires an absolute Docker path, workspace, pinned image, private network and execution name".to_owned() }
        };
        let image = self.image.ok_or_else(invalid)?;
        let digest = image
            .rsplit_once("sha256:")
            .map(|(_, digest)| digest)
            .ok_or_else(invalid)?;
        let network = self.network.ok_or_else(invalid)?;
        let name = self.name.ok_or_else(invalid)?;
        #[cfg(unix)]
        let runtime_user = {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(&self.directory)?;
            if metadata.uid() == 0 {
                return Err(SchedulerError::config_error(
                    "Evaluator supervisor must not run as root",
                ));
            }
            format!("{}:{}", metadata.uid(), metadata.gid())
        };
        #[cfg(not(unix))]
        let runtime_user = "1001:1001".to_owned();
        if digest.len() != 64
            || !digest.bytes().all(|c| c.is_ascii_hexdigit())
            || !self.docker.is_absolute()
            || !self.directory.is_absolute()
            || self.directory.to_string_lossy().contains(',')
            || !name.starts_with("eval-")
            || !safe_name(&name)
            || !safe_name(&network)
            || matches!(network.as_str(), "host" | "bridge" | "default" | "none")
            || !safe_name(&self.output_stem)
            || !safe_label(&self.owner_label)
            || !safe_label(&self.execution_label)
        {
            return Err(invalid());
        }
        Ok(ContainerLaunch {
            docker: self.docker,
            directory: self.directory,
            image,
            network,
            name,
            output_stem: self.output_stem,
            owner_label: self.owner_label,
            execution_label: self.execution_label,
            runtime_user,
        })
    }
}

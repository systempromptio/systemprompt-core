//! Owned client containers with bounded output and explicit cancellation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::client::{ClientPurpose, NativeClient};
use crate::{SchedulerError, SchedulerResult};
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
        let output = self.directory.join(format!("{}-events.jsonl", self.output_stem));
        let log = private_log(&output)?;
        let errors_path = self.directory.join(format!("{}-stderr.log", self.output_stem));
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
        command.arg(&self.image).args(client.arguments_for(purpose, prompt));
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
                message: "Client exceeded execution time, output, or writable-storage limit".to_owned(),
            });
        }
        Ok(self.child.try_wait()?)
    }

    fn directory(&self) -> SchedulerResult<PathBuf> {
        Ok(self.output.parent().ok_or_else(|| SchedulerError::config_error("Evaluator output path has no workspace parent"))?.join("home"))
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
        for entry in std::fs::read_dir(directory)? {
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
        self.output_stem = output_stem.into(); self
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
        let digest = image.rsplit_once("sha256:").map(|(_, digest)| digest).ok_or_else(invalid)?;
        let network = self.network.ok_or_else(invalid)?;
        let name = self.name.ok_or_else(invalid)?;
        #[cfg(unix)]
        let runtime_user = {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(&self.directory)?;
            if metadata.uid() == 0 { return Err(SchedulerError::config_error("Evaluator supervisor must not run as root")); }
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
    pub fn create(docker: PathBuf, name: String, owner: &str, execution: &str) -> SchedulerResult<Self> {
        if !docker.is_absolute() || !safe_name(&name) || !safe_label(owner) || !safe_label(execution) || matches!(name.as_str(), "host" | "bridge" | "default" | "none") {
            return Err(SchedulerError::config_error("Invalid evaluator network configuration"));
        }
        docker_status(&docker, &["network", "create", "--internal", "--label", "systemprompt.evaluator=true", "--label", &format!("systemprompt.evaluator.owner={owner}"), "--label", &format!("systemprompt.evaluator.execution={execution}"), &name])?;
        let network = Self { docker, name, relay: None, removed: false, owner_label: owner.to_owned(), execution_label: execution.to_owned() };
        network.verify(&[])?;
        Ok(network)
    }

    pub fn name(&self) -> &str { &self.name }

    pub fn start_relay(&mut self, image: &str, control_network: &str, upstream: &str, name: String) -> SchedulerResult<()> {
        if !safe_name(control_network) || matches!(control_network, "host" | "bridge" | "default" | "none") || !safe_name(&name) || !image.contains("@sha256:") {
            return Err(SchedulerError::config_error("Relay requires pinned image and dedicated control network"));
        }
        docker_status(&self.docker, &["run", "-d", "--name", &name, "--label", "systemprompt.evaluator=true", "--label", &format!("systemprompt.evaluator.owner={}", self.owner_label), "--label", &format!("systemprompt.evaluator.execution={}", self.execution_label), "--network", &self.name, "--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges", "--pids-limit=64", "--memory=256m", "--cpus=.25", "--user=1002:1002", "-e", &format!("SYSTEMPROMPT_RELAY_UPSTREAM={upstream}"), image])?;
        docker_status(&self.docker, &["network", "connect", control_network, &name])?;
        self.relay = Some(name.clone());
        self.verify(&[name])
    }

    pub fn verify(&self, expected: &[String]) -> SchedulerResult<()> {
        let output = Command::new(&self.docker).args(["network", "inspect", &self.name]).output()?;
        if !output.status.success() { return Err(SchedulerError::config_error("Execution network inspection failed")); }
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| SchedulerError::Internal(error.to_string()))?;
        let network = value.as_array().and_then(|values| values.first()).ok_or_else(|| SchedulerError::config_error("Execution network inspection returned no network"))?;
        if network.get("Internal").and_then(serde_json::Value::as_bool) != Some(true) { return Err(SchedulerError::config_error("Execution network is not internal")); }
        let mut actual = network.get("Containers").and_then(serde_json::Value::as_object).into_iter().flat_map(|containers| containers.values()).filter_map(|container| container.get("Name").and_then(serde_json::Value::as_str)).map(str::to_owned).collect::<Vec<_>>();
        let mut expected = expected.to_vec(); actual.sort(); expected.sort();
        if actual != expected { return Err(SchedulerError::config_error("Execution network has unexpected members")); }
        Ok(())
    }

    pub fn cleanup(&mut self) -> SchedulerResult<()> {
        if let Some(relay) = self.relay.take() { docker_status(&self.docker, &["rm", "--force", &relay])?; }
        docker_status(&self.docker, &["network", "rm", &self.name])?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for ExecutionNetwork {
    fn drop(&mut self) {
        if !self.removed {
            if let Some(relay) = self.relay.take() { let _ = docker_status(&self.docker, &["rm", "--force", &relay]); }
            let _ = docker_status(&self.docker, &["network", "rm", &self.name]);
        }
    }
}

fn docker_status(docker: &Path, arguments: &[&str]) -> SchedulerResult<()> {
    let status = Command::new(docker).args(arguments).stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    if !status.success() { return Err(SchedulerError::config_error(format!("Docker {} failed", arguments.first().copied().unwrap_or("command")))); }
    Ok(())
}

fn safe_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

fn safe_label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 255 && value.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

fn private_log(path: &Path) -> std::io::Result<std::fs::File> {
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
fn wait_bounded(child: &mut Child) -> std::io::Result<ExitStatus> {
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

//! Owned client containers with bounded output and explicit cancellation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::client::NativeClient;
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
}

#[derive(Debug)]
pub struct ContainerLaunch {
    docker: PathBuf,
    image: String,
    network: String,
    name: String,
    directory: PathBuf,
}

impl ContainerLaunch {
    pub const fn builder(docker: PathBuf, directory: PathBuf) -> ContainerLaunchBuilder {
        ContainerLaunchBuilder {
            docker,
            directory,
            image: None,
            network: None,
            name: None,
        }
    }

    pub fn start(
        &self,
        client: &NativeClient,
        prompt: &str,
    ) -> SchedulerResult<ContainerExecution> {
        let output = self.directory.join("client-events.jsonl");
        let log = private_log(&output)?;
        let errors_path = self.directory.join("client-stderr.log");
        let errors = private_log(&errors_path)?;
        let mut command = Command::new(&self.docker);
        command.args([
            "run",
            "--rm",
            "--name",
            &self.name,
            "--label",
            "systemprompt.evaluator=true",
            "--network",
            &self.network,
            "--read-only",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--pids-limit=128",
            "--memory=2g",
            "--cpus=1",
            "--user=1001:1001",
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
        command.arg(&self.image).args(client.arguments(prompt));
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
        })
    }
}

impl ContainerExecution {
    pub fn poll(&mut self) -> SchedulerResult<Option<ExitStatus>> {
        if self.started.elapsed().as_secs() > u64::from(self.timeout_seconds)
            || std::fs::metadata(&self.output)?
                .len()
                .saturating_add(std::fs::metadata(&self.errors)?.len())
                > self.max_output_bytes
        {
            self.cancel()?;
            return Err(SchedulerError::ConfigError {
                message: "Client exceeded execution time or output limit".to_owned(),
            });
        }
        Ok(self.child.try_wait()?)
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

#[derive(Debug)]
pub struct ContainerLaunchBuilder {
    docker: PathBuf,
    directory: PathBuf,
    image: Option<String>,
    network: Option<String>,
    name: Option<String>,
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
    pub fn build(self) -> SchedulerResult<ContainerLaunch> {
        let invalid = || {
            SchedulerError::ConfigError { message: "Evaluator requires an absolute Docker path, workspace, pinned image, private network and execution name".to_owned() }
        };
        let image = self.image.ok_or_else(invalid)?;
        let digest = image.strip_prefix("sha256:").ok_or_else(invalid)?;
        let network = self.network.ok_or_else(invalid)?;
        let name = self.name.ok_or_else(invalid)?;
        if digest.len() != 64
            || !digest.bytes().all(|c| c.is_ascii_hexdigit())
            || !self.docker.is_absolute()
            || !self.directory.is_absolute()
            || self.directory.to_string_lossy().contains(',')
            || !name.starts_with("eval-")
            || !safe_name(&name)
            || !safe_name(&network)
            || matches!(network.as_str(), "host" | "bridge" | "default" | "none")
        {
            return Err(invalid());
        }
        Ok(ContainerLaunch {
            docker: self.docker,
            directory: self.directory,
            image,
            network,
            name,
        })
    }
}

fn safe_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
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

fn wait_bounded(child: &mut Child) -> std::io::Result<ExitStatus> {
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

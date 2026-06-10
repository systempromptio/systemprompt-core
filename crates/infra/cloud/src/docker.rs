//! Docker CLI invocations behind a stubbable process-execution seam.
//!
//! [`DockerCli`] is the single place the platform shells out to `docker`. The
//! deploy-oriented operations (build, login, push) carry their own error
//! mapping; the raw [`DockerCli::output`] / [`DockerCli::status`] primitives
//! exist for callers that interpret exit codes and captured output themselves
//! (container inspection, compose lifecycle, `docker exec psql`). All process
//! execution flows through [`CommandRunner`], so tests can substitute a stub
//! instead of spawning real processes.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

use crate::error::{CloudError, CloudResult};

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
}

impl CommandSpec {
    pub fn docker<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            program: "docker".to_owned(),
            args: args.into_iter().map(Into::into).collect(),
            current_dir: None,
        }
    }

    #[must_use]
    pub fn rendered(&self) -> String {
        format!("{} {}", self.program, self.args.join(" "))
    }
}

pub trait CommandRunner: Send + Sync {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output>;
    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus>;
    fn status_with_stdin(&self, spec: &CommandSpec, stdin: &[u8]) -> io::Result<ExitStatus>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCommandRunner;

impl SystemCommandRunner {
    fn command(spec: &CommandSpec) -> Command {
        let mut command = Command::new(&spec.program);
        command.args(&spec.args);
        if let Some(dir) = &spec.current_dir {
            command.current_dir(dir);
        }
        command
    }
}

impl CommandRunner for SystemCommandRunner {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output> {
        Self::command(spec).output()
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        Self::command(spec).status()
    }

    fn status_with_stdin(&self, spec: &CommandSpec, stdin: &[u8]) -> io::Result<ExitStatus> {
        let mut command = Self::command(spec);
        command.stdin(Stdio::piped());
        let mut child = command.spawn()?;
        if let Some(mut handle) = child.stdin.take() {
            io::Write::write_all(&mut handle, stdin)?;
        }
        child.wait()
    }
}

pub struct DockerCli {
    runner: Box<dyn CommandRunner>,
}

impl std::fmt::Debug for DockerCli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerCli").finish_non_exhaustive()
    }
}

impl Default for DockerCli {
    fn default() -> Self {
        Self::new()
    }
}

impl DockerCli {
    #[must_use]
    pub fn new() -> Self {
        Self {
            runner: Box::new(SystemCommandRunner),
        }
    }

    #[must_use]
    pub const fn with_runner(runner: Box<dyn CommandRunner>) -> Self {
        Self { runner }
    }

    pub fn build_image(
        &self,
        context_dir: &Path,
        dockerfile: &Path,
        image: &str,
    ) -> CloudResult<()> {
        let dockerfile_arg = dockerfile.to_string_lossy().into_owned();
        let mut spec = CommandSpec::docker([
            "build",
            "--no-cache",
            "-f",
            dockerfile_arg.as_str(),
            "-t",
            image,
            ".",
        ]);
        spec.current_dir = Some(context_dir.to_path_buf());

        let status = self.runner.status(&spec).map_err(|e| {
            CloudError::docker_with(format!("Failed to run: {}", spec.rendered()), e)
        })?;

        if !status.success() {
            return Err(CloudError::docker(format!(
                "Command failed: {}",
                spec.rendered()
            )));
        }

        Ok(())
    }

    pub fn login(&self, registry: &str, username: &str, token: &str) -> CloudResult<()> {
        let spec = CommandSpec::docker(["login", registry, "-u", username, "--password-stdin"]);

        let status = self
            .runner
            .status_with_stdin(&spec, token.as_bytes())
            .map_err(|e| {
                CloudError::docker_with(format!("failed to spawn `docker login {registry}`"), e)
            })?;

        if !status.success() {
            return Err(CloudError::docker("Docker login failed"));
        }

        Ok(())
    }

    pub fn push(&self, image: &str) -> CloudResult<()> {
        let spec = CommandSpec::docker(["push", image]);

        let status = self.runner.status(&spec).map_err(|e| {
            CloudError::docker_with(format!("failed to spawn `docker push {image}`"), e)
        })?;

        if !status.success() {
            return Err(CloudError::docker(format!(
                "Docker push failed for image: {}",
                image
            )));
        }

        Ok(())
    }

    pub fn output(&self, args: &[&str]) -> io::Result<Output> {
        self.runner
            .output(&CommandSpec::docker(args.iter().copied()))
    }

    pub fn status(&self, args: &[&str]) -> io::Result<ExitStatus> {
        self.runner
            .status(&CommandSpec::docker(args.iter().copied()))
    }
}

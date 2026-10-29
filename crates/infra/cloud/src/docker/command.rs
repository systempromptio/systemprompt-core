//! The process-execution seam [`super::DockerCli`] runs every command through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

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

    pub fn git<I, S>(repo: &Path, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            program: "git".to_owned(),
            args: args.into_iter().map(Into::into).collect(),
            current_dir: Some(repo.to_path_buf()),
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

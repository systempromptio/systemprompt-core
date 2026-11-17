//! The process-execution seam [`super::DockerCli`] runs every command through.
//! The system runner spawns each one as a recorded foreground child, so an
//! interrupted CLI command stops it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::sync::Arc;

use systemprompt_extension::cli::ChildSupervisor;

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

#[derive(Clone)]
pub struct SystemCommandRunner {
    children: Arc<dyn ChildSupervisor>,
}

impl std::fmt::Debug for SystemCommandRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemCommandRunner")
            .finish_non_exhaustive()
    }
}

impl SystemCommandRunner {
    #[must_use]
    pub fn new(children: Arc<dyn ChildSupervisor>) -> Self {
        Self { children }
    }

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
        self.children.output(Self::command(spec))
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        self.children.status(Self::command(spec))
    }

    fn status_with_stdin(&self, spec: &CommandSpec, stdin: &[u8]) -> io::Result<ExitStatus> {
        self.children.status_with_stdin(Self::command(spec), stdin)
    }
}

//! Administrator rights: whether the process holds them, and a request to run
//! one program with them.
//!
//! A request always goes through the system's own prompt (the UAC consent
//! dialog, the macOS authorization dialog, the polkit agent), so nothing is
//! elevated without the person at the machine seeing it. Where a system has no
//! prompt to show, the request ends as [`ElevationOutcome::Manual`] with the
//! command an administrator runs by hand. A caller that is already elevated
//! runs the program itself and does not come here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::{Path, PathBuf};

use crate::platform::{Current, System};
use crate::quote;

/// Whether the process already holds administrator rights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    Elevated,
    Standard,
}

/// One program to run with administrator rights, with the reason the person
/// asked to approve it is shown where the system's prompt carries one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevatedCommand {
    program: String,
    arguments: Vec<String>,
    reason: String,
}

impl ElevatedCommand {
    pub fn new(
        program: &Path,
        arguments: Vec<String>,
        reason: &str,
    ) -> Result<Self, ElevationError> {
        let Some(text) = quote::program_text(program) else {
            return Err(ElevationError::Program {
                path: program.to_path_buf(),
            });
        };
        if let Some(index) = arguments.iter().position(|a| !quote::is_plain(a)) {
            return Err(ElevationError::Argument { index });
        }
        if reason.trim().is_empty() || !quote::is_plain(reason) {
            return Err(ElevationError::Reason);
        }
        Ok(Self {
            program: text.to_owned(),
            arguments,
            reason: reason.to_owned(),
        })
    }

    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// How a request for administrator rights ended.
///
/// `Declined` is the person refusing the prompt. `Manual` is a system with no
/// prompt to show: nothing ran, and `command` is what an administrator runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElevationOutcome {
    Succeeded,
    Failed { code: Option<i32>, detail: String },
    Declined,
    Manual { command: String },
}

/// Why administrator rights could not be read or requested.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ElevationError {
    #[error("{} is not an absolute path of plain text", path.display())]
    Program { path: PathBuf },
    #[error("argument {index} holds a control character")]
    Argument { index: usize },
    #[error("an elevation request needs a reason of plain text")]
    Reason,
    #[error("read the elevation of the process: {source}")]
    Detect {
        #[source]
        source: io::Error,
    },
    #[error("start {command}: {source}")]
    Spawn {
        command: String,
        #[source]
        source: io::Error,
    },
    #[error("wait for the elevated program: {source}")]
    Wait {
        #[source]
        source: io::Error,
    },
}

pub fn current() -> Result<Elevation, ElevationError> {
    Current::elevation()
}

pub fn request(command: &ElevatedCommand) -> Result<ElevationOutcome, ElevationError> {
    Current::elevate(command)
}

#[must_use]
pub fn authorization_script(command: &ElevatedCommand) -> String {
    let line = quote::posix_command_line(command.program(), command.arguments());
    format!(
        "do shell script \"{}\" with prompt \"{}\" with administrator privileges",
        quote::applescript_text(&line),
        quote::applescript_text(command.reason()),
    )
}

#[must_use]
pub fn manual_command(command: &ElevatedCommand) -> String {
    format!(
        "sudo -- {}",
        quote::posix_command_line(command.program(), command.arguments())
    )
}

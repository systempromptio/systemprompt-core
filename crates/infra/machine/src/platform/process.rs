//! Running a system tool to completion, with no console window and no
//! standard input.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsStr;
use std::io;
#[cfg(windows)]
use std::os::windows::process::CommandExt as _;
use std::process::{Command, Stdio};

// Why: a process of the Windows GUI subsystem owns no console, so a console
// child without this flag opens a window and takes the foreground.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug)]
pub(super) struct Finished {
    pub(super) code: Option<i32>,
    pub(super) stderr: String,
}

impl Finished {
    pub(super) fn succeeded(&self) -> bool {
        self.code == Some(0)
    }
}

pub(super) fn run<I, S>(program: &OsStr, arguments: I) -> io::Result<Finished>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command.args(arguments).stdin(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output()?;
    Ok(Finished {
        code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

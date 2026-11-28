//! The file-mode contract of runtime files: setting the mode a planned file
//! is written with, and checking it on the way back.
//!
//! [`Platform`] is the contract and each supported system implements it once:
//! Unix (0644, or 0755 when executable) and Windows (no mode bits). `Host`
//! names the implementation for the system being built, so a system without
//! one does not compile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::{fs, io};

use systemprompt_models::feedback::receipts::ReadbackStatus;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix::Unix as Host;
#[cfg(windows)]
use windows::Windows as Host;

trait Platform {
    fn set_mode(path: &Path, executable: bool) -> io::Result<()>;

    fn mode_check(metadata: Option<&fs::Metadata>, executable: bool) -> ReadbackStatus;
}

pub(super) fn set_mode(path: &Path, executable: bool) -> io::Result<()> {
    Host::set_mode(path, executable)
}

pub(super) fn mode_check(metadata: Option<&fs::Metadata>, executable: bool) -> ReadbackStatus {
    Host::mode_check(metadata, executable)
}

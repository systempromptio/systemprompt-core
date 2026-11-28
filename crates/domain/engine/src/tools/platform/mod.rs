//! The case rule of path containment: Windows names files without regard to
//! case, so two settled paths are compared in lower case there, and as written
//! everywhere else. `Host` names the implementation for the system being
//! built, so a system without one does not compile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix::Unix as Host;
#[cfg(windows)]
use windows::Windows as Host;

trait Platform {
    fn comparable(settled: PathBuf) -> PathBuf;
}

pub(super) fn comparable(settled: PathBuf) -> PathBuf {
    Host::comparable(settled)
}

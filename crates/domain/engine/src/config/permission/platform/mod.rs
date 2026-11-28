//! The case rule of rule matching: Windows names files without regard to
//! case, so a rule and its subject are compared in lower case there, and as
//! written everywhere else. `Host` names the implementation for the system
//! being built, so a system without one does not compile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix::Unix as Host;
#[cfg(windows)]
use windows::Windows as Host;

trait Platform {
    fn fold(chars: impl Iterator<Item = char>) -> Vec<char>;
}

pub(super) fn fold(chars: impl Iterator<Item = char>) -> Vec<char> {
    Host::fold(chars)
}

//! Windows install swap: the running image is displaced to a unique sibling
//! name and the new binary takes its path (see [`crate::update::swap`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::update::error::UpdateError;
use crate::update::install::{probe_writable, running_exe};
use crate::update::swap;

pub(super) fn preflight() -> Result<PathBuf, UpdateError> {
    let target = running_exe()?;
    probe_writable(
        &target,
        "reinstall to a per-user location, or re-run as Administrator",
    )?;
    Ok(target)
}

pub(super) fn apply(staged: &Path, target: PathBuf) -> Result<PathBuf, UpdateError> {
    swap::swap(staged, &target, &swap::SwapNonce::generate())?;
    Ok(target)
}

pub(super) fn sweep_leftovers() {
    let Ok(exe) = running_exe() else {
        return;
    };
    swap::sweep(&exe);
}

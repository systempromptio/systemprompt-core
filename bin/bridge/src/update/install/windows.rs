//! Windows install swap.
//!
//! Windows holds an image-section lock on a running `.exe`, so it cannot be
//! overwritten — but it *can* be renamed, because the lock is on the file
//! object rather than the directory entry. The upgrade is therefore: move the
//! running binary aside, write the new one at the original path, and delete the
//! leftover on the next start.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::update::error::UpdateError;
use crate::update::install::{probe_writable, running_exe};

/// Suffix for the displaced binary. Swept on the next start, once the lock the
/// previous process held is gone.
const OLD_SUFFIX: &str = ".old";

pub(super) fn apply(staged: &Path) -> Result<PathBuf, UpdateError> {
    let target = running_exe()?;
    probe_writable(
        &target,
        "reinstall to a per-user location, or re-run as Administrator",
    )?;

    let displaced = displaced_path(&target);
    if displaced.exists() {
        std::fs::remove_file(&displaced).map_err(|e| UpdateError::io(&displaced, e))?;
    }

    std::fs::rename(&target, &displaced).map_err(|e| UpdateError::io(&target, e))?;

    // Any failure past this point leaves no usable binary at `target`, so put
    // the running one back rather than leaving a half-updated install.
    if let Err(e) = std::fs::copy(staged, &target).map_err(|e| UpdateError::io(&target, e)) {
        if let Err(restore) = std::fs::rename(&displaced, &target) {
            tracing::error!(
                error = %restore,
                path = %target.display(),
                "update: install failed AND rollback failed; the previous binary is at the .old path"
            );
        }
        return Err(e);
    }

    tracing::info!(path = %target.display(), "update: binary replaced");
    Ok(target)
}

fn displaced_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(OLD_SUFFIX);
    PathBuf::from(name)
}

/// Deletes the previous binary left behind by an earlier update. Best-effort:
/// on the very first run after a swap the old process may still be exiting, in
/// which case the delete fails and the next start retries it.
pub(super) fn sweep_leftovers() {
    let Ok(exe) = running_exe() else {
        return;
    };
    let displaced = displaced_path(&exe);
    if !displaced.exists() {
        return;
    }
    match std::fs::remove_file(&displaced) {
        Ok(()) => tracing::info!(path = %displaced.display(), "update: removed previous binary"),
        Err(e) => {
            tracing::debug!(error = %e, path = %displaced.display(), "update: previous binary still locked; will retry next start");
        },
    }
}

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

const OLD_SUFFIX: &str = ".old";
const NEW_SUFFIX: &str = ".new";

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

    // Why: the new image is copied beside the target first; a partial copy
    // then leaves the running binary untouched, and the swap itself is two
    // renames, which the image-section lock permits and which never leave the
    // original path empty for longer than the second rename.
    let incoming = incoming_path(&target);
    if let Err(e) = std::fs::copy(staged, &incoming) {
        crate::fsutil::remove_leftover_file(&incoming);
        return Err(UpdateError::io(&incoming, e));
    }
    std::fs::rename(&target, &displaced).map_err(|e| UpdateError::io(&target, e))?;
    if let Err(e) = std::fs::rename(&incoming, &target) {
        crate::fsutil::remove_leftover_file(&incoming);
        if let Err(restore) = std::fs::rename(&displaced, &target) {
            tracing::error!(
                error = %restore,
                path = %target.display(),
                "update: install failed AND rollback failed; the previous binary is at the .old path"
            );
        }
        return Err(UpdateError::io(&target, e));
    }

    tracing::info!(path = %target.display(), "update: binary replaced");
    Ok(target)
}

fn displaced_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(OLD_SUFFIX);
    PathBuf::from(name)
}

fn incoming_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(NEW_SUFFIX);
    PathBuf::from(name)
}

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

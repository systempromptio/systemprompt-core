//! Linux install swap.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::update::error::UpdateError;
use crate::update::install::{probe_writable, running_exe};

pub(super) fn apply(staged: &Path) -> Result<PathBuf, UpdateError> {
    let target = running_exe()?;
    probe_writable(
        &target,
        "re-run with sudo, or install to a directory you own such as ~/.local/bin",
    )?;

    let workdir = staged.with_extension("unpack");
    if workdir.exists() {
        _ = std::fs::remove_dir_all(&workdir);
    }
    std::fs::create_dir_all(&workdir).map_err(|e| UpdateError::io(&workdir, e))?;

    let extracted = unpack(staged, &workdir)?;
    let result = swap(&extracted, &target);
    if let Err(e) = std::fs::remove_dir_all(&workdir) {
        tracing::debug!(error = %e, path = %workdir.display(), "update: unpack cleanup failed");
    }
    result.map(|()| target)
}

// Why: shells out to `tar` rather than linking an archive crate — it is present
// on every supported distribution, and this mirrors `scripts/install-bridge.sh`.
fn unpack(archive: &Path, into: &Path) -> Result<PathBuf, UpdateError> {
    let out = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .output()
        .map_err(|e| UpdateError::Unpack(format!("could not run tar: {e}")))?;
    if !out.status.success() {
        return Err(UpdateError::Unpack(format!(
            "tar failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    find_binary(into, crate::brand::brand().binary_name).ok_or_else(|| {
        UpdateError::Unpack(format!(
            "the archive contains no {} executable",
            crate::brand::brand().binary_name
        ))
    })
}

fn find_binary(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        } else if path.file_name().is_some_and(|n| n == name) {
            return Some(path);
        }
    }
    subdirs.iter().find_map(|d| find_binary(d, name))
}

// Why: writes beside the target then renames over it — overwriting a running
// binary in place fails with `ETXTBSY`, which is exactly the upgrade case. The
// rename is atomic and the running process keeps its original inode.
fn swap(new_binary: &Path, target: &Path) -> Result<(), UpdateError> {
    let staged_next = crate::fsutil::temp_path_for(target);
    std::fs::copy(new_binary, &staged_next).map_err(|e| UpdateError::io(&staged_next, e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&staged_next, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| UpdateError::io(&staged_next, e))?;
    }

    std::fs::rename(&staged_next, target).map_err(|e| {
        _ = std::fs::remove_file(&staged_next);
        UpdateError::io(target, e)
    })?;
    tracing::info!(path = %target.display(), "update: binary replaced");
    Ok(())
}

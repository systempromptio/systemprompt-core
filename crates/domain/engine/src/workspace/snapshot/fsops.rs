//! Writing and removing worktree files during a restore without following
//! symlinks, with read-only files made writable for the change and put back
//! afterwards.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::{fs, io};

use crate::workspace::platform;

pub(crate) enum Written {
    Yes,
    SkippedSymlink,
}

pub(crate) fn write_file(path: &Path, bytes: &[u8], executable: bool) -> io::Result<Written> {
    let mut was_readonly = false;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => return Ok(Written::SkippedSymlink),
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir(path).map_err(|error| {
                tracing::debug!(
                    path = %path.display(),
                    %error,
                    "directory is where a file belongs"
                );
                io::Error::other("a non-empty directory is where the snapshot has a file")
            })?;
        },
        Ok(metadata) => {
            if metadata.permissions().readonly() {
                platform::make_writable(path, &metadata.permissions())?;
                was_readonly = true;
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
        },
        Err(error) => return Err(error),
    }
    fs::write(path, bytes)?;
    platform::set_executable(path, executable)?;
    if was_readonly {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions)?;
    }
    Ok(Written::Yes)
}

pub(crate) fn remove_file(root: &Path, path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || metadata.is_dir() {
                return Ok(false);
            }
            if metadata.permissions().readonly() {
                platform::make_writable(path, &metadata.permissions())?;
            }
        },
    }
    fs::remove_file(path)?;
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir == root || !dir.starts_with(root) || fs::remove_dir(dir).is_err() {
            break;
        }
        current = dir.parent();
    }
    Ok(true)
}

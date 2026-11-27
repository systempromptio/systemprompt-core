//! Writing and removing worktree files during a restore without following
//! symlinks, with read-only files made writable for the change and put back
//! afterwards.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::{fs, io};

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
                make_writable(path, &metadata.permissions())?;
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
    set_executable(path, executable)?;
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
                make_writable(path, &metadata.permissions())?;
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

#[cfg(unix)]
fn make_writable(path: &Path, permissions: &fs::Permissions) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(permissions.mode() | 0o200))
}

#[cfg(not(unix))]
fn make_writable(path: &Path, permissions: &fs::Permissions) -> io::Result<()> {
    let mut permissions = permissions.clone();
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "Windows has one read-only attribute; clearing it is the only way to rewrite or \
                  delete the file"
    )]
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)?.permissions().mode();
    let wanted = if executable {
        mode | ((mode & 0o444) >> 2)
    } else {
        mode & !0o111
    };
    if wanted != mode {
        fs::set_permissions(path, fs::Permissions::from_mode(wanted))?;
    }
    Ok(())
}

#[cfg(not(unix))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the Unix counterpart changes permissions and can fail"
)]
const fn set_executable(_path: &Path, _executable: bool) -> io::Result<()> {
    Ok(())
}

//! Owner-only files on macOS and Linux: mode `0600` for a file and `0700`
//! for a directory, owned by the user the process runs as, and never reached
//! through a symbolic link (`O_NOFOLLOW` on every open).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::{self, File, Metadata};
use std::io;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;

use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::unistd::Uid;

use crate::secure::SecureError;

const FILE_MODE: u32 = 0o600;
const DIRECTORY_MODE: u32 = 0o700;
const MODE_BITS: u32 = 0o777;

pub(super) fn is_link(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

pub(super) fn create_owner_only(path: &Path) -> Result<File, SecureError> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(FILE_MODE)
        .custom_flags(OFlag::O_NOFOLLOW.bits())
        .open(path)
        .map_err(|source| SecureError::io("create temporary file", path, source))?;
    file.set_permissions(fs::Permissions::from_mode(FILE_MODE))
        .map_err(|source| SecureError::io("restrict temporary file", path, source))?;
    Ok(file)
}

// Why: POSIX answers an `O_NOFOLLOW` open whose last component is a symbolic
// link with ELOOP, which `std::io::ErrorKind` does not name on stable.
pub(super) fn open_unfollowed(path: &Path) -> Result<Option<File>, SecureError> {
    let opened = fs::OpenOptions::new()
        .read(true)
        .custom_flags(OFlag::O_NOFOLLOW.bits())
        .open(path);
    match opened {
        Ok(file) => Ok(Some(file)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) if e.raw_os_error() == Some(Errno::ELOOP as i32) => Err(SecureError::Link {
            path: path.to_path_buf(),
        }),
        Err(source) => Err(SecureError::io("read", path, source)),
    }
}

pub(super) fn replace(staged: &Path, path: &Path) -> Result<(), SecureError> {
    fs::rename(staged, path).map_err(|source| SecureError::io("replace", path, source))?;
    let Some(parent) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) else {
        return Ok(());
    };
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|source| SecureError::io("flush directory", parent, source))
}

pub(super) fn verify_owner_only(path: &Path) -> Result<(), SecureError> {
    let metadata = unfollowed_metadata(path)?;
    if !metadata.is_file() {
        return Err(access(path, "not a regular file"));
    }
    verify_private(path, &metadata, FILE_MODE)
}

pub(super) fn restrict_directory(path: &Path) -> Result<(), SecureError> {
    fs::set_permissions(path, fs::Permissions::from_mode(DIRECTORY_MODE))
        .map_err(|source| SecureError::io("restrict directory", path, source))?;
    let metadata = unfollowed_metadata(path)?;
    if !metadata.is_dir() {
        return Err(access(path, "not a directory"));
    }
    verify_private(path, &metadata, DIRECTORY_MODE)
}

fn unfollowed_metadata(path: &Path) -> Result<Metadata, SecureError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| SecureError::io("inspect", path, source))?;
    if is_link(&metadata) {
        return Err(SecureError::Link {
            path: path.to_path_buf(),
        });
    }
    Ok(metadata)
}

fn verify_private(path: &Path, metadata: &Metadata, mode: u32) -> Result<(), SecureError> {
    if metadata.mode() & MODE_BITS != mode {
        return Err(access(path, "mode is not owner-only"));
    }
    if metadata.uid() != Uid::effective().as_raw() {
        return Err(access(path, "owned by another user"));
    }
    Ok(())
}

fn access(path: &Path, reason: &'static str) -> SecureError {
    SecureError::Acl {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::PermissionDenied, reason),
    }
}

//! The public write, read and remove operations: a staged temporary file beside
//! the target, replaced atomically, then read back and checked for content and
//! access.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use super::{Access, WriteOutcome};
use crate::platform::{Native, Platform};
use crate::secure::error::SecureError;

pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), SecureError> {
    write_atomic(path, bytes, Access::Private)
}

pub fn write_shared(path: &Path, bytes: &[u8]) -> Result<(), SecureError> {
    write_atomic(path, bytes, Access::Shared)
}

pub fn write_atomic(path: &Path, bytes: &[u8], access: Access) -> Result<(), SecureError> {
    let parent = parent_dir(path)?;
    if !parent.exists() {
        match access {
            Access::Private => ensure_private_dir(parent)?,
            Access::Shared => fs::create_dir_all(parent)
                .map_err(|source| SecureError::io("create directory", parent, source))?,
        }
    }
    refuse_link(parent)?;
    refuse_link(path)?;
    let temp = temp_path(parent, path)?;
    let staged = stage(&temp, bytes, access).and_then(|()| Native::replace(&temp, path));
    if let Err(e) = staged {
        remove_leftover(&temp);
        return Err(e);
    }
    verify_contents(path, bytes)?;
    verify_access(path, access)
}

pub fn write_if_changed(
    path: &Path,
    bytes: &[u8],
    access: Access,
) -> Result<WriteOutcome, SecureError> {
    refuse_link(path)?;
    match fs::read(path) {
        Ok(current) if current == bytes && verify_access(path, access).is_ok() => {
            return Ok(WriteOutcome::Unchanged);
        },
        Ok(_) => {},
        Err(e) if e.kind() == io::ErrorKind::NotFound => {},
        Err(source) => return Err(SecureError::io("read", path, source)),
    }
    write_atomic(path, bytes, access)?;
    Ok(WriteOutcome::Written)
}

pub fn remove_if_exists(path: &Path) -> Result<bool, SecureError> {
    let removed = match fs::remove_file(path) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(source) => return Err(SecureError::io("remove", path, source)),
    };
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(removed),
        Err(source) => Err(SecureError::io("inspect after removing", path, source)),
        Ok(_) => Err(SecureError::StillPresent {
            path: path.to_path_buf(),
        }),
    }
}

pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, SecureError> {
    refuse_link(path)?;
    let Some(mut file) = Native::open_unfollowed(path)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    io::copy(&mut file, &mut bytes).map_err(|source| SecureError::io("read", path, source))?;
    Ok(Some(bytes))
}

pub fn ensure_private_dir(path: &Path) -> Result<(), SecureError> {
    fs::create_dir_all(path).map_err(|source| SecureError::io("create directory", path, source))?;
    refuse_link(path)?;
    Native::restrict_directory(path)
}

fn refuse_link(path: &Path) -> Result<(), SecureError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if Native::is_link(&metadata) => Err(SecureError::Link {
            path: path.to_path_buf(),
        }),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SecureError::io("inspect", path, source)),
    }
}

fn verify_access(path: &Path, access: Access) -> Result<(), SecureError> {
    match access {
        Access::Private => Native::verify_owner_only(path),
        Access::Shared => Ok(()),
    }
}

fn create_staged(temp: &Path, access: Access) -> Result<File, SecureError> {
    match access {
        Access::Private => Native::create_owner_only(temp),
        Access::Shared => fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp)
            .map_err(|source| SecureError::io("create temporary file", temp, source)),
    }
}

fn parent_dir(path: &Path) -> Result<&Path, SecureError> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            SecureError::io(
                "resolve the directory of",
                path,
                io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory"),
            )
        })
}

fn temp_path(parent: &Path, path: &Path) -> Result<PathBuf, SecureError> {
    let name = path.file_name().ok_or_else(|| {
        SecureError::io(
            "name a temporary file for",
            path,
            io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"),
        )
    })?;
    let mut temp = std::ffi::OsString::from(".");
    temp.push(name);
    temp.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    Ok(parent.join(temp))
}

fn stage(temp: &Path, bytes: &[u8], access: Access) -> Result<(), SecureError> {
    let mut file = create_staged(temp, access)?;
    file.write_all(bytes)
        .map_err(|source| SecureError::io("write", temp, source))?;
    file.sync_all()
        .map_err(|source| SecureError::io("flush", temp, source))?;
    drop(file);
    Ok(())
}

fn verify_contents(path: &Path, expected: &[u8]) -> Result<(), SecureError> {
    let actual = fs::read(path).map_err(|source| SecureError::io("read back", path, source))?;
    if actual == expected {
        Ok(())
    } else {
        Err(SecureError::ReadbackMismatch {
            path: path.to_path_buf(),
        })
    }
}

// Why: a leftover temporary file cannot change the result, but it is usually
// the first sign of a permission problem, so it is reported rather than
// ignored.
fn remove_leftover(temp: &Path) {
    if let Err(e) = fs::remove_file(temp)
        && e.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, path = %temp.display(), "temporary file was not removed");
    }
}

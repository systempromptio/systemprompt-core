//! The platform-specific parts of a write: link refusal, owner-only creation of
//! the staged file, the replacing rename, and verification of the resulting
//! access.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::{self, File};
use std::io;
use std::path::Path;

use super::Access;
use crate::secure::error::SecureError;

pub(super) fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    if crate::secure::win::is_reparse_point(metadata) {
        return true;
    }
    metadata.file_type().is_symlink()
}

pub(super) fn refuse_link(path: &Path) -> Result<(), SecureError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link(&metadata) => Err(SecureError::Link {
            path: path.to_path_buf(),
        }),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SecureError::io("inspect", path, source)),
    }
}

#[cfg(windows)]
pub(super) fn create_temp(temp: &Path, access: Access) -> Result<File, SecureError> {
    match access {
        Access::Private => {
            let owner = crate::secure::win::current_sid().map_err(|source| SecureError::Acl {
                path: temp.to_path_buf(),
                source,
            })?;
            crate::secure::win::create_private(temp, &owner)
                .map_err(|source| SecureError::io("create private temporary file", temp, source))
        },
        Access::Shared => fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp)
            .map_err(|source| SecureError::io("create temporary file", temp, source)),
    }
}

#[cfg(not(windows))]
pub(super) fn create_temp(temp: &Path, access: Access) -> Result<File, SecureError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if access == Access::Private {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let file = options
        .open(temp)
        .map_err(|source| SecureError::io("create temporary file", temp, source))?;
    #[cfg(unix)]
    if access == Access::Private {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| SecureError::io("restrict temporary file", temp, source))?;
    }
    Ok(file)
}

// Why: Windows refuses to replace a file another handle holds open without
// FILE_SHARE_DELETE (an antivirus scan, the search indexer), and those handles
// close within moments.
#[cfg(windows)]
pub(super) fn replace(temp: &Path, path: &Path) -> Result<(), SecureError> {
    const BACKOFF_MS: [u64; 5] = [50, 80, 120, 160, 200];
    for delay in BACKOFF_MS {
        match crate::secure::win::move_replace(temp, path) {
            Ok(()) => return Ok(()),
            Err(e) if crate::secure::win::is_transient_replace_error(&e) => {
                tracing::debug!(
                    error = %e,
                    path = %path.display(),
                    delay_ms = delay,
                    "replace refused; retrying"
                );
                std::thread::sleep(std::time::Duration::from_millis(delay));
            },
            Err(source) => return Err(SecureError::io("replace", path, source)),
        }
    }
    crate::secure::win::move_replace(temp, path)
        .map_err(|source| SecureError::io("replace after retries", path, source))
}

#[cfg(not(windows))]
pub(super) fn replace(temp: &Path, path: &Path) -> Result<(), SecureError> {
    fs::rename(temp, path).map_err(|source| SecureError::io("replace", path, source))
}

pub(super) fn verify_access(path: &Path, access: Access) -> Result<(), SecureError> {
    if access == Access::Shared {
        return Ok(());
    }
    #[cfg(windows)]
    {
        let acl_error = |source: io::Error| SecureError::Acl {
            path: path.to_path_buf(),
            source,
        };
        let owner = crate::secure::win::current_sid().map_err(acl_error)?;
        let file = File::open(path).map_err(|source| SecureError::io("open", path, source))?;
        crate::secure::win::verify_private(&file, &owner).map_err(acl_error)?;
    }
    #[cfg(unix)]
    verify_mode(path, 0o600)?;
    Ok(())
}

#[cfg(unix)]
pub(super) fn verify_mode(path: &Path, mode: u32) -> Result<(), SecureError> {
    use std::os::unix::fs::PermissionsExt as _;
    let metadata = fs::metadata(path).map_err(|source| SecureError::io("inspect", path, source))?;
    if metadata.permissions().mode() & 0o777 == mode {
        Ok(())
    } else {
        Err(SecureError::Acl {
            path: path.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("mode is not {mode:o}"),
            ),
        })
    }
}

#[cfg(unix)]
pub(super) fn sync_directory(parent: &Path) -> Result<(), SecureError> {
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|source| SecureError::io("flush directory", parent, source))
}

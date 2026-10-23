//! Verified removal and best-effort cleanup of files and directories.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::{fs, io};

pub fn remove_verified(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => {},
        Err(e) if e.kind() == io::ErrorKind::NotFound => {},
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!("remove {}: {e}", path.display()),
            ));
        },
    }
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(_) => Err(io::Error::other(format!(
            "{} still exists after deletion",
            path.display()
        ))),
    }
}

// Why: temporary probes and staging trees are cleaned up on a best-effort
// basis; a leftover cannot change the installed result, but it must not be
// silent either, because a directory that cannot be removed is usually the
// first sign of a permission problem the next step will hit.
pub fn remove_leftover_file(path: &Path) {
    if let Err(e) = fs::remove_file(path)
        && e.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, path = %path.display(), "leftover file was not removed");
    }
}

pub fn remove_leftover_dir(path: &Path) {
    if let Err(e) = fs::remove_dir_all(path)
        && e.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, path = %path.display(), "leftover directory was not removed");
    }
}

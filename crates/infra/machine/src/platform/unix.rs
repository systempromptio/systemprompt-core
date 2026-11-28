//! What macOS and Linux share: the current user from the account database,
//! files replaced in one step, and a bounded read of a policy file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::{self, File, Metadata};
use std::io::{self, Read as _};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use nix::unistd::{Uid, User};

use crate::elevation::Elevation;
use crate::policy::{DOCUMENT_LIMIT, PolicyError};

const OTHERS_MAY_WRITE: u32 = 0o022;

pub(super) fn elevation() -> Elevation {
    if Uid::effective().is_root() {
        Elevation::Elevated
    } else {
        Elevation::Standard
    }
}

pub(super) fn user() -> io::Result<User> {
    User::from_uid(Uid::current())
        .map_err(io::Error::from)?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "the current user has no entry in the account database",
            )
        })
}

pub(super) fn write_replacing(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory")
    })?;
    fs::create_dir_all(parent)?;
    let mut staged = path.as_os_str().to_owned();
    staged.push(format!(".{}.staged", std::process::id()));
    let staged = PathBuf::from(staged);
    fs::write(&staged, contents)?;
    if let Err(error) = fs::rename(&staged, path) {
        if let Err(cleanup) = fs::remove_file(&staged) {
            tracing::debug!(
                path = %staged.display(),
                error = %cleanup,
                "staged file left behind after a failed replace"
            );
        }
        return Err(error);
    }
    Ok(())
}

pub(super) fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        Ok(()) | Err(_) => Ok(()),
    }
}

pub(super) fn read_policy_file(
    path: &Path,
    trusted: impl FnOnce(&Metadata) -> bool,
) -> Result<Option<Vec<u8>>, PolicyError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(PolicyError::io("open", path, source)),
    };
    let metadata = file
        .metadata()
        .map_err(|source| PolicyError::io("inspect", path, source))?;
    if !metadata.is_file() || !trusted(&metadata) {
        return Err(PolicyError::Untrusted {
            path: path.to_path_buf(),
        });
    }
    let mut bytes = Vec::new();
    file.take(DOCUMENT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| PolicyError::io("read", path, source))?;
    if bytes.len() as u64 > DOCUMENT_LIMIT {
        return Err(PolicyError::TooLarge {
            path: path.to_path_buf(),
            limit: DOCUMENT_LIMIT,
        });
    }
    Ok(Some(bytes))
}

pub(super) fn root_owned(metadata: &Metadata) -> bool {
    metadata.uid() == 0 && metadata.mode() & OTHERS_MAY_WRITE == 0
}

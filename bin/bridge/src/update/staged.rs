//! The record of a release that has been swapped over the install but is not
//! yet running.
//!
//! The record is written after a successful install and read by every update
//! check, so a release already on disk is reported as waiting for a restart
//! instead of being downloaded and installed again. It names the program file
//! the install wrote and that file's SHA-256: a record whose program file no
//! longer hashes to the recorded digest (replaced, reverted or removed by
//! hand) is not honoured. Only the process holding the single-instance lock
//! clears it, once the running build is at least the recorded version, so a
//! short-lived invocation of the new binary never clears a record an older
//! running instance still relies on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use super::error::UpdateError;
use crate::release_version::is_newer;

const MARKER_FILE: &str = "staged-update.json";

const HASH_CHUNK: usize = 64 * 1024;

/// A release installed on disk that the running process has not started yet.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StagedUpdate {
    pub version: String,
    pub program: PathBuf,
    pub sha256: String,
}

#[must_use]
pub fn marker_path() -> Option<PathBuf> {
    crate::config::paths::bridge_metadata_dir().map(|dir| dir.join(MARKER_FILE))
}

pub fn hash_program(program: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(program)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_CHUNK];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(buffer.get(..read).unwrap_or_default());
    }
    Ok(crate::hash::hex_encode(&hasher.finalize()))
}

#[must_use]
pub fn read(path: &Path) -> Option<StagedUpdate> {
    let raw = match crate::fsutil::read_private(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "update: staged-update record unreadable");
            return None;
        },
    };
    match serde_json::from_slice::<StagedUpdate>(&raw) {
        Ok(staged) => Some(staged),
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "update: staged-update record malformed; ignoring it");
            None
        },
    }
}

pub fn record(path: &Path, staged: &StagedUpdate) -> Result<(), UpdateError> {
    let bytes = serde_json::to_vec(staged).map_err(UpdateError::StagedRecordEncode)?;
    crate::fsutil::atomic_write_0600(path, &bytes).map_err(|e| UpdateError::io(path, e))?;
    if read(path).as_ref() != Some(staged) {
        return Err(UpdateError::StagedRecordUnverified {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

pub fn record_installed(
    path: &Path,
    version: &str,
    program: &Path,
) -> Result<StagedUpdate, UpdateError> {
    let sha256 = hash_program(program).map_err(|e| UpdateError::io(program, e))?;
    let staged = StagedUpdate {
        version: version.to_owned(),
        program: program.to_path_buf(),
        sha256,
    };
    record(path, &staged)?;
    Ok(staged)
}

#[must_use]
pub fn pending(path: &Path, running: &str) -> Option<StagedUpdate> {
    read(path).filter(|staged| is_newer(&staged.version, running) && on_disk(staged))
}

fn on_disk(staged: &StagedUpdate) -> bool {
    match hash_program(&staged.program) {
        Ok(actual) if actual == staged.sha256 => true,
        Ok(_) => {
            tracing::info!(
                staged = %staged.version,
                program = %staged.program.display(),
                "update: the installed program no longer matches the staged release; ignoring the record"
            );
            false
        },
        Err(e) => {
            tracing::info!(
                error = %e,
                staged = %staged.version,
                program = %staged.program.display(),
                "update: the staged program file cannot be read; ignoring the record"
            );
            false
        },
    }
}

pub fn reconcile(path: &Path, running: &str) -> Result<Option<StagedUpdate>, UpdateError> {
    let Some(staged) = read(path) else {
        return clear_unreadable(path).map(|()| None);
    };
    if is_newer(&staged.version, running) && on_disk(&staged) {
        return Ok(Some(staged));
    }
    crate::fsutil::remove_verified(path).map_err(|e| UpdateError::io(path, e))?;
    tracing::info!(
        staged = %staged.version,
        running,
        "update: the staged release is running or no longer installed; record cleared"
    );
    Ok(None)
}

fn clear_unreadable(path: &Path) -> Result<(), UpdateError> {
    if !path.exists() {
        return Ok(());
    }
    crate::fsutil::remove_verified(path).map_err(|e| UpdateError::io(path, e))
}

#[must_use]
pub fn pending_for_running() -> Option<StagedUpdate> {
    marker_path().and_then(|path| pending(&path, crate::brand::brand().version))
}

pub fn reconcile_for_running() {
    let Some(marker) = marker_path() else {
        return;
    };
    if let Err(e) = reconcile(&marker, crate::brand::brand().version) {
        tracing::warn!(error = %e, path = %marker.display(), "update: staged-update record could not be reconciled");
    }
}

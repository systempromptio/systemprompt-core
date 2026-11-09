//! Replacing a program file that may be the running image.
//!
//! Windows holds an image-section lock on a running `.exe`: it cannot be
//! overwritten or deleted, but it can be renamed, because the lock is on the
//! file object rather than the directory entry. The swap therefore never
//! deletes anything at the target: the new file is copied to
//! `<exe>.<nonce>.new`, the current file moves to `<exe>.<nonce>.old`, and the
//! new one takes its place. [`sweep`] removes this exe's own leftovers of both
//! kinds, plus the `<exe>.old` earlier releases wrote, once nothing runs from
//! them; any other file next to the exe is never touched.
//!
//! The functions take every path as an argument, so the sequence is the same
//! on every platform and is exercised off Windows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::{Path, PathBuf};

use super::error::UpdateError;

const OLD_SUFFIX: &str = ".old";
const NEW_SUFFIX: &str = ".new";
const NONCE_LEN: usize = 32;

/// The token that names one swap's displaced and incoming files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapNonce(String);

impl SwapNonce {
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().simple().to_string())
    }

    #[must_use]
    pub fn parse(candidate: &str) -> Option<Self> {
        (candidate.len() == NONCE_LEN
            && candidate
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| Self(candidate.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn sibling(target: &Path, nonce: &SwapNonce, suffix: &str) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(".");
    name.push(nonce.as_str());
    name.push(suffix);
    PathBuf::from(name)
}

#[must_use]
pub fn displaced_path(target: &Path, nonce: &SwapNonce) -> PathBuf {
    sibling(target, nonce, OLD_SUFFIX)
}

#[must_use]
pub fn incoming_path(target: &Path, nonce: &SwapNonce) -> PathBuf {
    sibling(target, nonce, NEW_SUFFIX)
}

pub fn swap(staged: &Path, target: &Path, nonce: &SwapNonce) -> Result<PathBuf, UpdateError> {
    swap_with(staged, target, nonce, &|from: &Path, to: &Path| {
        std::fs::rename(from, to)
    })
}

pub fn swap_with(
    staged: &Path,
    target: &Path,
    nonce: &SwapNonce,
    rename: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> Result<PathBuf, UpdateError> {
    let displaced = displaced_path(target, nonce);
    let incoming = incoming_path(target, nonce);
    if let Err(e) = std::fs::copy(staged, &incoming) {
        crate::fsutil::remove_leftover_file(&incoming);
        return Err(UpdateError::io(&incoming, e));
    }
    if let Err(source) = rename(target, &displaced) {
        crate::fsutil::remove_leftover_file(&incoming);
        return Err(replace_blocked(target, source));
    }
    if let Err(source) = rename(&incoming, target) {
        crate::fsutil::remove_leftover_file(&incoming);
        if let Err(restore) = rename(&displaced, target) {
            return Err(UpdateError::RollbackRestore {
                failure: Box::new(replace_blocked(target, source)),
                backup: displaced,
                source: restore,
            });
        }
        return Err(replace_blocked(target, source));
    }
    tracing::info!(
        path = %target.display(),
        displaced = %displaced.display(),
        "update: binary replaced"
    );
    Ok(displaced)
}

fn replace_blocked(target: &Path, source: io::Error) -> UpdateError {
    UpdateError::ReplaceBlocked {
        dir: target
            .parent()
            .map_or_else(|| target.to_path_buf(), Path::to_path_buf),
        source,
    }
}

#[must_use]
pub fn leftovers(target: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (target.parent(), target.file_name()) else {
        return Vec::new();
    };
    let name = name.to_string_lossy().into_owned();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .is_some_and(|n| is_leftover_of(&name, &n.to_string_lossy()))
        })
        .collect()
}

fn is_leftover_of(exe: &str, candidate: &str) -> bool {
    let Some(rest) = candidate
        .strip_prefix(exe)
        .and_then(|rest| rest.strip_prefix('.'))
    else {
        return false;
    };
    if rest == &OLD_SUFFIX[1..] {
        return true;
    }
    [OLD_SUFFIX, NEW_SUFFIX].iter().any(|suffix| {
        rest.strip_suffix(suffix)
            .is_some_and(|nonce| SwapNonce::parse(nonce).is_some())
    })
}

pub fn sweep(target: &Path) -> Vec<PathBuf> {
    let mut kept = Vec::new();
    for leftover in leftovers(target) {
        match std::fs::remove_file(&leftover) {
            Ok(()) => {
                tracing::info!(path = %leftover.display(), "update: removed a previous update file");
            },
            Err(e) => {
                tracing::debug!(error = %e, path = %leftover.display(), "update: previous binary still in use; left for a later start");
                kept.push(leftover);
            },
        }
    }
    kept
}

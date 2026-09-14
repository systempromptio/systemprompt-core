//! Strictly relative manifest file paths on every platform the bridge runs on.
//!
//! Manifest paths are joined under a staging directory, so a path the gateway
//! signs must be relative even where Linux's `Path::components` would read a
//! Windows drive prefix or UNC root as an ordinary segment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path, PathBuf};

use super::ApplyError;

pub fn manifest_relative(raw: &str) -> Result<PathBuf, ApplyError> {
    let unsafe_path = || ApplyError::UnsafePath(raw.to_owned());
    if raw.is_empty() || raw.starts_with(['/', '\\']) {
        return Err(unsafe_path());
    }
    let normalised = raw.replace('\\', "/");
    let segments_ok = normalised.split('/').all(|segment| {
        !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(':')
    });
    if !segments_ok {
        return Err(unsafe_path());
    }
    let path = PathBuf::from(&normalised);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(unsafe_path());
    }
    Ok(path)
}

#[must_use]
pub fn is_manifest_relative(raw: &str) -> bool {
    manifest_relative(raw).is_ok()
}

pub(super) fn join_under(stage: &Path, raw: &str) -> Result<PathBuf, ApplyError> {
    manifest_relative(raw).map(|relative| stage.join(relative))
}

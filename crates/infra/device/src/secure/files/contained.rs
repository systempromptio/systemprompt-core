//! Resolution of a manifest-supplied relative path (`/`-separated) below a
//! trusted root. Anything that could land outside the root or be read
//! differently on Windows is refused: absolute paths, empty, `.` and `..`
//! segments, backslashes, drive or stream colons, characters Windows
//! forbids, trailing dots or spaces, and reserved device names. Every existing
//! component below the root must not be a symlink or reparse point, and every
//! one but the last must be a directory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::platform::{Native, Platform};
use crate::secure::error::SecureError;

pub fn contained_path(root: &Path, relative: &str) -> Result<PathBuf, SecureError> {
    let escapes = |reason: &'static str| SecureError::Escapes {
        root: root.to_path_buf(),
        relative: relative.to_owned(),
        reason,
    };
    if relative.is_empty() {
        return Err(escapes("the path is empty"));
    }
    if relative.starts_with('/') {
        return Err(escapes("the path is absolute"));
    }
    let segments: Vec<&str> = relative.split('/').collect();
    for segment in &segments {
        check_segment(segment).map_err(escapes)?;
    }
    let mut current = root.to_path_buf();
    let last = segments.len().saturating_sub(1);
    for (index, segment) in segments.iter().enumerate() {
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if Native::is_link(&metadata) {
                    return Err(SecureError::Link { path: current });
                }
                if index < last && !metadata.is_dir() {
                    return Err(escapes("a parent component is not a directory"));
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(source) => return Err(SecureError::io("inspect", current, source)),
        }
    }
    Ok(segments
        .iter()
        .fold(root.to_path_buf(), |path, segment| path.join(segment)))
}

pub(crate) fn check_segment(segment: &str) -> Result<(), &'static str> {
    if segment.is_empty() {
        return Err("the path has an empty segment");
    }
    if segment == "." || segment == ".." {
        return Err("the path has a relative segment");
    }
    if segment
        .chars()
        .any(|c| c.is_control() || matches!(c, '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Err("the path has a character Windows does not allow in a name");
    }
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Err("a segment ends with a dot or a space");
    }
    if is_reserved_device(segment) {
        return Err("a segment is a reserved Windows device name");
    }
    Ok(())
}

fn is_reserved_device(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or(segment);
    let stem = stem.trim_end_matches(' ').to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let bytes = stem.as_bytes();
    bytes.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && bytes.get(3).is_some_and(|d| matches!(d, b'1'..=b'9'))
}

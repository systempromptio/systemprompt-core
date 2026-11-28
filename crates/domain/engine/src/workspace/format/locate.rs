//! Finding where a formatter applies: root markers, project-local shims and
//! argument expansion.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::workspace::platform;

const FILE_PLACEHOLDER: &str = "$FILE";

pub(super) fn find_marker(
    start: &Path,
    markers: &[String],
    boundary: Option<&Path>,
) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if markers.iter().any(|marker| has_marker(dir, marker)) {
            return Some(dir.to_path_buf());
        }
        if boundary == Some(dir) {
            break;
        }
    }
    None
}

fn has_marker(dir: &Path, marker: &str) -> bool {
    marker.strip_prefix('*').map_or_else(
        || dir.join(marker).exists(),
        |suffix| has_suffix_entry(dir, suffix),
    )
}

fn has_suffix_entry(dir: &Path, suffix: &str) -> bool {
    let suffix = suffix.to_ascii_lowercase();
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.to_ascii_lowercase().ends_with(&suffix))
        })
    })
}

pub(super) fn local_bin(start: &Path, command: &str, boundary: Option<&Path>) -> Option<PathBuf> {
    let names = platform::shim_names(command);
    for dir in start.ancestors() {
        let bin = dir.join("node_modules").join(".bin");
        if let Some(found) = names
            .iter()
            .map(|name| bin.join(name))
            .find(|p| p.is_file())
        {
            return Some(found);
        }
        if boundary == Some(dir) {
            break;
        }
    }
    None
}

pub(super) fn expand_args(args: &[String], file: &Path) -> Vec<OsString> {
    let mut out = Vec::with_capacity(args.len() + 1);
    let mut named = false;
    for arg in args {
        if arg == FILE_PLACEHOLDER {
            out.push(file.as_os_str().to_owned());
            named = true;
        } else if arg.contains(FILE_PLACEHOLDER) {
            out.push(OsString::from(
                arg.replace(FILE_PLACEHOLDER, &file.to_string_lossy()),
            ));
            named = true;
        } else {
            out.push(OsString::from(arg));
        }
    }
    if !named {
        out.push(file.as_os_str().to_owned());
    }
    out
}

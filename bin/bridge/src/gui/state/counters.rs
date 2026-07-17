//! Monotonic counters surfaced in GUI state snapshots.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::config::paths;
use std::path::Path;
use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_DIRS, PLUGIN_MANIFEST_FILE};

pub(super) fn count_plugin_dirs(root: &Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().ok().is_some_and(|t| t.is_dir()) {
            n += 1;
        }
    }
    Some(n)
}

pub(super) fn count_malformed_plugin_dirs(root: &Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if !entry.file_type().ok().is_some_and(|t| t.is_dir()) {
            continue;
        }
        let path = entry.path();
        let well_formed = PLUGIN_MANIFEST_DIRS
            .iter()
            .any(|dir| path.join(dir).join(PLUGIN_MANIFEST_FILE).is_file());
        if !well_formed && name != paths::SYNTHETIC_PLUGIN_NAME {
            n += 1;
        }
    }
    Some(n)
}

pub(super) fn count_dir_children(dir: &Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().ok().is_some_and(|t| t.is_dir()) {
            n += 1;
        }
    }
    Some(n)
}

pub(super) fn count_md_files(dir: &Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if !entry.file_type().ok().is_some_and(|t| t.is_file()) {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            n += 1;
        }
    }
    Some(n)
}

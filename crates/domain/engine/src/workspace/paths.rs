//! Worktree-relative keys: forward-slash strings that name a file the same way
//! on every platform, compared case-insensitively where the file system is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path, PathBuf};

pub(crate) fn key_from_relative(rel: &Path) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {},
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

pub(crate) fn rel_key_under(root: &Path, path: &Path) -> Option<String> {
    key_from_relative(path.strip_prefix(root).ok()?)
}

pub(crate) fn relative_key(root: &Path, path: &Path) -> Option<String> {
    if path.is_relative() {
        return key_from_relative(path);
    }
    let mut path_parts = path.components();
    for root_part in root.components() {
        let path_part = path_parts.next()?;
        if !same_component(root_part, path_part) {
            return None;
        }
    }
    key_from_relative(path_parts.as_path())
}

fn same_component(a: Component<'_>, b: Component<'_>) -> bool {
    fold(&a.as_os_str().to_string_lossy()) == fold(&b.as_os_str().to_string_lossy())
}

pub(crate) fn path_for_key(root: &Path, key: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for segment in key.split('/') {
        let unsafe_segment = segment.is_empty()
            || segment == "."
            || segment == ".."
            || (cfg!(windows) && segment.contains(['\\', ':']));
        if unsafe_segment {
            return None;
        }
        path.push(segment);
    }
    Some(path)
}

pub(crate) fn relative_path(key: &str) -> PathBuf {
    key.split('/').collect()
}

pub(crate) fn key_matches(key: &str, requested: &str) -> bool {
    let key = fold(key);
    let requested = fold(requested);
    key == requested
        || key
            .strip_prefix(requested.as_str())
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(windows)]
pub(crate) fn fold(value: &str) -> String {
    value.to_lowercase()
}

#[cfg(not(windows))]
pub(crate) fn fold(value: &str) -> String {
    value.to_owned()
}

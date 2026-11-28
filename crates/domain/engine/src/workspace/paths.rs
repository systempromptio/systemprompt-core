//! Worktree-relative keys: forward-slash strings that name a file the same way
//! on every platform, compared case-insensitively where the file system is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path, PathBuf};

use crate::workspace::platform;

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

const VERBATIM: [&str; 2] = [r"\\?\", "//?/"];
const VERBATIM_UNC: &str = "UNC";

// Why: a worktree and a file path name the same place whether either carries a
// Windows verbatim prefix (`\\?\`) or has had its separators turned into
// forward slashes, so both are compared as plain segments of text.
pub(crate) fn relative_key(root: &Path, path: &Path) -> Option<String> {
    if path.is_relative() {
        return key_from_relative(path);
    }
    let root_text = root.to_string_lossy();
    let path_text = path.to_string_lossy();
    let mut path_parts = segments(&path_text);
    for root_part in segments(&root_text) {
        if fold(path_parts.next()?) != fold(root_part) {
            return None;
        }
    }
    let rest: PathBuf = path_parts.collect();
    key_from_relative(&rest)
}

fn segments(text: &str) -> impl Iterator<Item = &str> {
    without_verbatim(text)
        .split(platform::separators())
        .filter(|segment| !segment.is_empty() && *segment != ".")
}

fn without_verbatim(text: &str) -> &str {
    VERBATIM
        .iter()
        .find_map(|prefix| text.strip_prefix(prefix))
        .map_or(text, |rest| match rest.get(..VERBATIM_UNC.len()) {
            Some(head) if head.eq_ignore_ascii_case(VERBATIM_UNC) => &rest[VERBATIM_UNC.len()..],
            _ => rest,
        })
}

pub(crate) fn path_for_key(root: &Path, key: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for segment in key.split('/') {
        let unsafe_segment = segment.is_empty()
            || segment == "."
            || segment == ".."
            || platform::leaves_directory(segment);
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

pub(crate) fn fold(value: &str) -> String {
    platform::fold(value)
}

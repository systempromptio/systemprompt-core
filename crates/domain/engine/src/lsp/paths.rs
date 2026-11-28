//! Lexical path normalisation and lookup of server binaries on an injected
//! search path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use crate::lsp::platform;

const DEFAULT_PATH_EXT: &str = ".COM;.EXE;.BAT;.CMD";

pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {},
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                },
                Some(Component::RootDir | Component::Prefix(_)) => {},
                _ => out.push(".."),
            },
            other => out.push(other.as_os_str()),
        }
    }
    platform::settle(out)
}

pub(crate) fn absolutize(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize(path)
    } else {
        normalize(&base.join(path))
    }
}

pub(crate) fn display_relative(path: &Path, base: Option<&Path>) -> String {
    let shown = base
        .and_then(|base| path.strip_prefix(base).ok())
        .unwrap_or(path);
    shown.to_string_lossy().replace('\\', "/")
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProgramSearch<'a> {
    pub(crate) path: Option<&'a OsStr>,
    pub(crate) path_ext: Option<&'a OsStr>,
    pub(crate) base: &'a Path,
}

pub(crate) fn find_program(command: &str, search: &ProgramSearch<'_>) -> Option<PathBuf> {
    if command.is_empty() {
        return None;
    }
    let extensions = executable_extensions(search.path_ext);
    let as_path = Path::new(command);
    if as_path.is_absolute() || command.contains('/') || command.contains('\\') {
        let candidate = if as_path.is_absolute() {
            as_path.to_path_buf()
        } else {
            search.base.join(as_path)
        };
        return platform::runnable(&candidate, &extensions);
    }
    let dirs = search.path?;
    std::env::split_paths(dirs)
        .filter(|dir| !dir.as_os_str().is_empty())
        .find_map(|dir| platform::runnable(&dir.join(command), &extensions))
}

fn executable_extensions(path_ext: Option<&OsStr>) -> Vec<String> {
    let raw = path_ext.map_or_else(
        || DEFAULT_PATH_EXT.to_owned(),
        |value| value.to_string_lossy().into_owned(),
    );
    raw.split(';')
        .map(str::trim)
        .filter(|ext| !ext.is_empty())
        .map(|ext| {
            let lower = ext.to_ascii_lowercase();
            if lower.starts_with('.') {
                lower
            } else {
                format!(".{lower}")
            }
        })
        .collect()
}

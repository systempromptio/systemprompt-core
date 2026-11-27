//! Path policy: resolving the paths a tool is given against the session's
//! directories and asking before going outside the project.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use serde_json::json;

use crate::tools::contract::{PermissionRequest, ToolContext};
use crate::tools::error::ToolError;

pub fn resolve(cwd: &Path, input: &str) -> PathBuf {
    let trimmed = input.trim();
    let raw = Path::new(trimmed);
    let joined = if raw.is_absolute() || raw.has_root() {
        raw.to_path_buf()
    } else {
        cwd.join(raw)
    };
    normalize(&joined)
}

pub fn normalize(path: &Path) -> PathBuf {
    let simplified = dunce::simplified(path);
    let mut out = PathBuf::new();
    for component in simplified.components() {
        match component {
            Component::CurDir => {},
            Component::ParentDir => {
                let last = match out.components().next_back() {
                    Some(Component::Normal(_)) => Tail::Name,
                    Some(Component::RootDir | Component::Prefix(_)) => Tail::Root,
                    _ => Tail::Relative,
                };
                match last {
                    Tail::Name => {
                        out.pop();
                    },
                    Tail::Root => {},
                    Tail::Relative => out.push(".."),
                }
            },
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[derive(Clone, Copy)]
enum Tail {
    Name,
    Root,
    Relative,
}

pub(crate) fn settle(path: &Path) -> PathBuf {
    let normal = normalize(path);
    let mut existing: &Path = &normal;
    let mut rest: Vec<&OsStr> = Vec::new();
    loop {
        if let Ok(canonical) = dunce::canonicalize(existing) {
            let mut out = canonical;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                existing = parent;
            },
            _ => return normal,
        }
    }
}

fn comparable(path: &Path) -> PathBuf {
    let settled = settle(path);
    if cfg!(windows) {
        PathBuf::from(settled.to_string_lossy().to_lowercase())
    } else {
        settled
    }
}

pub fn is_within(path: &Path, root: &Path) -> bool {
    comparable(path).starts_with(comparable(root))
}

pub fn display_path(path: &Path, root: &Path) -> String {
    let settled = settle(path);
    let base = settle(root);
    match settled.strip_prefix(&base) {
        Ok(rel) if !rel.as_os_str().is_empty() => slash(rel),
        _ => slash(&settled),
    }
}

pub(crate) fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn dir_pattern(path: &Path) -> String {
    let dir = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    let text = slash(&settle(dir));
    format!("{}/*", text.trim_end_matches('/'))
}

pub async fn guard_external(ctx: &ToolContext, path: &Path, action: &str) -> Result<(), ToolError> {
    if is_within(path, &ctx.worktree) || is_within(path, &ctx.data_dir) {
        return Ok(());
    }
    let pattern = dir_pattern(path);
    let shown = slash(&settle(path));
    ctx.ask(PermissionRequest {
        permission: "external_directory".to_owned(),
        patterns: vec![pattern.clone()],
        always: vec![pattern.clone()],
        title: format!("{action} outside the project: {shown}"),
        metadata: json!({ "path": shown, "directory": pattern }),
    })
    .await
}

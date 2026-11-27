//! Resolving server definitions against the search path and choosing the
//! servers for a file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::builtins::BUILTINS;
use super::{LspConfig, ServerId, ServerSpec};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("command `{command}` was not found")]
pub(crate) struct MissingCommand {
    command: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Resolved {
    pub(crate) spec: Arc<ServerSpec>,
    pub(crate) program: Result<PathBuf, MissingCommand>,
}

pub(crate) fn resolve_servers(
    config: &LspConfig,
    locate: impl Fn(&str) -> Option<PathBuf>,
) -> Vec<Resolved> {
    let disabled: HashSet<&ServerId> = config.disabled.iter().collect();
    let configured: HashSet<&ServerId> = config.servers.iter().map(|spec| &spec.id).collect();
    let mut taken_groups: HashSet<&'static str> = BUILTINS
        .iter()
        .filter(|builtin| configured.contains(&ServerId::new(builtin.id)))
        .filter_map(|builtin| builtin.group)
        .collect();

    let mut resolved = Vec::new();
    let mut seen: HashSet<&ServerId> = HashSet::new();
    for spec in &config.servers {
        if disabled.contains(&spec.id) || !seen.insert(&spec.id) {
            continue;
        }
        let program = locate(&spec.command).ok_or_else(|| MissingCommand {
            command: spec.command.clone(),
        });
        resolved.push(Resolved {
            spec: Arc::new(spec.clone()),
            program,
        });
    }

    if config.builtins {
        for builtin in BUILTINS {
            let id = ServerId::new(builtin.id);
            if configured.contains(&id) || disabled.contains(&id) {
                continue;
            }
            if builtin
                .group
                .is_some_and(|group| taken_groups.contains(group))
            {
                continue;
            }
            let Some(program) = locate(builtin.command) else {
                continue;
            };
            if let Some(group) = builtin.group {
                taken_groups.insert(group);
            }
            resolved.push(Resolved {
                spec: Arc::new(builtin.spec()),
                program: Ok(program),
            });
        }
    }
    resolved
}

fn root_for(spec: &ServerSpec, file: &Path, worktree: &Path) -> Option<PathBuf> {
    if !file.starts_with(worktree) || file == worktree {
        return None;
    }
    let start = file.parent()?;
    for marker in &spec.root_markers {
        let mut dir = start;
        loop {
            if marker_present(dir, marker) {
                return Some(dir.to_path_buf());
            }
            if dir == worktree {
                break;
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }
    }
    Some(worktree.to_path_buf())
}

fn marker_present(dir: &Path, marker: &str) -> bool {
    marker.strip_prefix('*').map_or_else(
        || dir.join(marker).exists(),
        |suffix| {
            std::fs::read_dir(dir).is_ok_and(|entries| {
                entries
                    .flatten()
                    .any(|entry| entry.file_name().to_string_lossy().ends_with(suffix))
            })
        },
    )
}

pub(crate) fn select<'a>(
    servers: &'a [Resolved],
    file: &Path,
    worktree: &Path,
) -> Vec<(&'a Resolved, PathBuf)> {
    servers
        .iter()
        .filter(|server| server.spec.handles(file))
        .filter_map(|server| root_for(&server.spec, file, worktree).map(|root| (server, root)))
        .collect()
}

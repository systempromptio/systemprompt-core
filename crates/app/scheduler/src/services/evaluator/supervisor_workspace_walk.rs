//! Bounded regular-file traversal shared by workspace integrity and evidence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Path, SchedulerError, SchedulerResult, internal};
use std::io::Read;

pub(super) fn visit_workspace(
    root: &Path,
    directory: &Path,
    visitor: &mut impl FnMut(String, &[u8], bool) -> SchedulerResult<()>,
) -> SchedulerResult<()> {
    visit_workspace_bounded(root, directory, visitor, &mut (0, 0), 0)
}

fn visit_workspace_bounded(
    root: &Path,
    directory: &Path,
    visitor: &mut impl FnMut(String, &[u8], bool) -> SchedulerResult<()>,
    budget: &mut (usize, u64),
    depth: usize,
) -> SchedulerResult<()> {
    if depth > 32
        || std::fs::symlink_metadata(directory)?
            .file_type()
            .is_symlink()
    {
        return Err(SchedulerError::config_error(
            "Workspace depth or directory link is unsafe",
        ));
    }
    let mut entries = std::fs::read_dir(directory)?
        .take(4097)
        .collect::<Result<Vec<_>, _>>()?;
    budget.0 = budget
        .0
        .checked_add(entries.len())
        .ok_or_else(|| SchedulerError::config_error("Workspace entry overflow"))?;
    if budget.0 > 4096 {
        return Err(SchedulerError::config_error(
            "Workspace entry limit exceeded",
        ));
    }
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(SchedulerError::config_error(
                "Evaluation workspace contains a link",
            ));
        }
        if metadata.is_dir() {
            visit_workspace_bounded(root, &path, visitor, budget, depth + 1)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(internal)?
                .to_str()
                .ok_or_else(|| SchedulerError::config_error("Workspace path is not valid UTF-8"))?
                .replace('\\', "/");
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let executable = false;
            let remaining = (64 * 1024 * 1024u64).saturating_sub(budget.1);
            if metadata.len() > remaining {
                return Err(SchedulerError::config_error(
                    "Workspace byte limit exceeded",
                ));
            }
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(remaining + 1)
                .read_to_end(&mut bytes)?;
            budget.1 += bytes.len() as u64;
            if budget.1 > 64 * 1024 * 1024 {
                return Err(SchedulerError::config_error(
                    "Workspace byte limit exceeded",
                ));
            }
            visitor(relative, &bytes, executable)?;
        } else {
            return Err(SchedulerError::config_error(
                "Evaluation workspace contains a non-regular file",
            ));
        }
    }
    Ok(())
}

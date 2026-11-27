//! The filesystem side of an apply: managed skills, owned bundle files and the
//! rules block, each written as projected, with whatever its ledger lists and
//! the projection no longer holds removed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::{fs, io};

use systemprompt_device::secure::{Access, contained_path, read_optional, remove_if_exists, write_if_changed};

use crate::projection::error::ProjectionError;
use crate::projection::ledger::{self, SKILL_FILE};
use crate::projection::model::{BundleFile, SkillSet};
use crate::projection::rules;

pub(crate) fn write_skills(
    root: &Path,
    ledger_path: &Path,
    skills: Option<&SkillSet>,
    previous: &[String],
) -> Result<(), ProjectionError> {
    let files = skills.map(|set| set.files.as_slice()).unwrap_or_default();
    for file in files {
        let path = contained_path(root, &format!("{}/{SKILL_FILE}", file.dir))
            .map_err(ProjectionError::secure("resolve a skill file"))?;
        let outcome = write_if_changed(&path, file.markdown.as_bytes(), Access::Shared)
            .map_err(ProjectionError::secure("write a skill"))?;
        tracing::debug!(dir = %file.dir, ?outcome, "managed skill");
    }
    for stale in previous
        .iter()
        .filter(|id| !files.iter().any(|file| &file.dir == *id))
    {
        if remove_tree(&root.join(stale))? {
            tracing::info!(dir = %stale, "managed skill removed");
        }
    }
    if let Some(set) = skills {
        let bytes = ledger::skills_bytes(set)?;
        let outcome = write_if_changed(ledger_path, &bytes, Access::Shared)
            .map_err(ProjectionError::secure("write the skills ledger"))?;
        tracing::debug!(?outcome, "skills ledger");
    } else {
        remove_if_exists(ledger_path)
            .map_err(ProjectionError::secure("remove the skills ledger"))?;
    }
    Ok(())
}

pub(crate) fn write_owned(
    config: &Path,
    ledger_path: &Path,
    files: &[BundleFile],
    previous: &[String],
) -> Result<(), ProjectionError> {
    for stale in previous
        .iter()
        .filter(|path| !files.iter().any(|file| &file.path == *path))
    {
        let path = contained_path(config, stale)
            .map_err(ProjectionError::secure("resolve an owned file"))?;
        if remove_if_exists(&path).map_err(ProjectionError::secure("remove an owned file"))? {
            tracing::info!(file = %stale, "owned file removed");
        }
    }
    for file in files {
        let path = contained_path(config, &file.path)
            .map_err(ProjectionError::secure("resolve an owned file"))?;
        let outcome = write_if_changed(&path, &file.bytes, Access::Shared)
            .map_err(ProjectionError::secure("write an owned file"))?;
        tracing::debug!(file = %file.path, ?outcome, "owned file");
    }
    let mut kept: Vec<String> = files.iter().map(|file| file.path.clone()).collect();
    kept.sort();
    kept.dedup();
    if kept.is_empty() {
        remove_if_exists(ledger_path)
            .map_err(ProjectionError::secure("remove the owned-files ledger"))?;
        return Ok(());
    }
    let bytes = ledger::owned_bytes(&kept)?;
    let outcome = write_if_changed(ledger_path, &bytes, Access::Shared)
        .map_err(ProjectionError::secure("write the owned-files ledger"))?;
    tracing::debug!(?outcome, "owned-files ledger");
    Ok(())
}

pub(crate) fn write_rules(path: &Path, block: Option<&str>) -> Result<(), ProjectionError> {
    let existing = match read_optional(path).map_err(ProjectionError::secure("read AGENTS.md"))? {
        Some(bytes) => String::from_utf8(bytes).map_err(|error| ProjectionError::NotUtf8 {
            path: path.to_path_buf(),
            source: error.utf8_error(),
        })?,
        None => String::new(),
    };
    let next = rules::splice(&existing, block);
    if next == existing {
        return Ok(());
    }
    if next.is_empty() {
        remove_if_exists(path).map_err(ProjectionError::secure("remove AGENTS.md"))?;
        tracing::info!(file = %path.display(), "rules block removed; nothing else was left");
        return Ok(());
    }
    let outcome = write_if_changed(path, next.as_bytes(), Access::Shared)
        .map_err(ProjectionError::secure("write AGENTS.md"))?;
    tracing::info!(file = %path.display(), ?outcome, rules = block.is_some(), "rules block written");
    Ok(())
}

fn remove_tree(path: &Path) -> Result<bool, ProjectionError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(ProjectionError::Io {
            op: "inspect",
            path: path.to_path_buf(),
            source,
        }),
        Ok(metadata) if metadata.is_file() => {
            remove_if_exists(path).map_err(ProjectionError::secure("remove a managed skill"))
        },
        Ok(_) => fs::remove_dir_all(path)
            .map(|()| true)
            .map_err(|source| ProjectionError::Io {
                op: "remove",
                path: path.to_path_buf(),
                source,
            }),
    }
}
